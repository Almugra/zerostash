#[macro_use]
extern crate serde_derive;

use std::path::PathBuf;
use std::sync::Mutex;

use directory::Dir;
use infinitree::*;

mod files;
pub mod directory;
pub use files::*;
pub use directory::*;
pub mod rollsum;
pub mod splitter;
mod stash;

pub use stash::restore;
pub use stash::store;

type ChunkIndex = fields::VersionedMap<Digest, ChunkPointer>;
type FileIndex = fields::VersionedMap<String, Entry>;
type DirectoryIndex = fields::VersionedMap<PathBuf, Mutex<Vec<Dir>>>;

#[derive(Clone, Default, Index)]
pub struct Files {
    pub chunks: ChunkIndex,
    pub files: FileIndex,
    pub directories: DirectoryIndex,
}
