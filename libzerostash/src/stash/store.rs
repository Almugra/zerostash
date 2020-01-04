use crate::chunks::ChunkIndex;
use crate::files::{self, FileIndex};
use crate::objects::*;
use crate::rollsum::SeaSplit;
use crate::splitter::FileSplitter;

use crossbeam_utils::thread;
use failure::Error;
use memmap::MmapOptions;
use walkdir::{DirEntry, WalkDir};

use std::fs;
use std::path::Path;

type Sender = crossbeam_channel::Sender<DirEntry>;
type Receiver = crossbeam_channel::Receiver<DirEntry>;

#[allow(unused)]
pub fn recursive(
    num_threads: usize,
    chunkindex: &mut (impl ChunkIndex),
    fileindex: &mut (impl FileIndex),
    objectstore: &mut (impl ObjectStore),
    path: impl AsRef<Path>,
) -> Result<(), Error> {
    thread::scope(|s| {
        let (sender, r) = crossbeam_channel::bounded::<DirEntry>(16 * num_threads);

        for i in 0..(num_threads - 1) {
            let receiver = r.clone();
            let chunkindex = chunkindex.clone();
            let fileindex = fileindex.clone();
            let objectstore = objectstore.clone();

            s.spawn(move |_| process_file_loop(receiver, chunkindex, fileindex, objectstore));
        }

        // we need sender to go out of scope
        // otherwise the channels never close
        process_path(num_threads, sender, path);
    })
    .map(|_| ())
    .map_err(|e| format_err!("threads: {:?}", e))
}

fn process_file_loop(
    receiver: Receiver,
    chunkindex: impl ChunkIndex,
    mut fileindex: impl FileIndex,
    mut objectstore: impl ObjectStore,
) {
    for file in receiver.iter() {
        let path = file.path();

        if file
            .path()
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            println!("skipping because contains parent {:?}", path);
            continue;
        }

        let osfile = fs::File::open(path).unwrap();
        let mut entry = files::Entry::from_file(&osfile, path).unwrap();

        if !fileindex.has_changed(&entry) {
            continue;
        }

        if entry.size == 0 {
            fileindex.push(entry);
            continue;
        }

        let mmap = unsafe {
            // avoid an unnecessary fstat() by passing `len`
            // directly from the previous call
            MmapOptions::new()
                .len(entry.size as usize)
                .map(&osfile)
                .unwrap()
        };

        for (start, hash, data) in FileSplitter::<SeaSplit>::new(&mmap).unwrap() {
            let chunkptr = chunkindex
                .push(hash, || objectstore.store_chunk(&hash, data))
                .unwrap();

            entry.chunks.push((start, chunkptr));
        }

        fileindex.push(entry);
    }

    objectstore.flush().unwrap();
}

fn process_path(threads: usize, sender: Sender, path: impl AsRef<Path>) {
    for entry in WalkDir::new(path.as_ref())
        .max_open(threads)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
    {
        sender.send(entry).unwrap();
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use crate::chunks::*;
    const PATH_100: &str = "tests/data/100_random_1k";

    #[test]
    fn test_stats_add_up() {
        use crate::chunks::*;
        use crate::files::*;
        use crate::objects::*;
        use crate::stash::store;

        let mut cs = RwLockIndex::new();
        let mut fs = HashMapFileIndex::default();
        let mut s = NullStorage::default();

        let _files = store::recursive(4, &mut cs, &mut fs, &mut s, PATH_100).unwrap();

        assert_eq!(100, fs.len());
        assert_eq!(1_024_000u64, fs.to_vec().iter().map(|f| f.size).sum());
    }

    fn bench_chunk_saturated_e2e<CS: ChunkIndex + 'static>(b: &mut test::Bencher) {
        use crate::files::*;
        use crate::objects::*;
        use crate::stash::store;

        let mut cs = CS::new();
        let mut os = NullStorage::default();
        let mut fs = HashMapFileIndex::default();

        // first build up the file index
        store::recursive(4, &mut cs, &mut fs, &mut os, PATH_100).unwrap();

        b.iter(|| {
            store::recursive(4, &mut cs, &mut fs, &mut os, PATH_100).unwrap();
        })
    }

    fn bench_chunk_e2e<CS: ChunkIndex + 'static>(b: &mut test::Bencher) {
        use crate::files::*;
        use crate::objects::*;
        use crate::stash::store;

        b.iter(|| {
            store::recursive(
                4,
                &mut CS::new(),
                &mut HashMapFileIndex::default(),
                &mut NullStorage::default(),
                PATH_100,
            )
            .unwrap()
        })
    }

    #[bench]
    fn bench_chunk_e2e_saturated_rwlock(b: &mut test::Bencher) {
        bench_chunk_saturated_e2e::<RwLockIndex>(b);
    }

    #[bench]
    fn bench_chunk_e2e_rwlock(b: &mut test::Bencher) {
        bench_chunk_e2e::<RwLockIndex>(b);
    }
}