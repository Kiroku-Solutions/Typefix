use fst::Map;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
type DictData = Arc<memmap2::Mmap>;
#[cfg(target_arch = "wasm32")]
type DictData = Arc<[u8]>;

#[derive(Clone)]
pub struct Dict {
    map: Map<DictData>,
    word_count: usize,
}

fn main() {}
