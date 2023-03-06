#[macro_use]
extern crate serde_derive;

use infinitree::*;
pub mod tree;
pub use tree::*;
mod files;
pub use files::*;
pub mod rollsum;
pub mod splitter;
mod stash;
pub use stash::restore;
pub use stash::store;

type ChunkIndex = fields::VersionedMap<Digest, ChunkPointer>;
type FileIndex = fields::VersionedMap<String, Entry>;

#[derive(Clone, Default, Index)]
pub struct Files {
    pub chunks: ChunkIndex,
    pub files: FileIndex,
    pub directory_tree: infinitree::fields::Serialized<Tree>,
}
