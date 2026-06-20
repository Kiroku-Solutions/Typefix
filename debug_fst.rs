use fst::{IntoStreamer, Streamer};
use fst::automaton::Levenshtein;

fn main() {
    let bytes = std::fs::read("data/dictionaries/es.fst").unwrap();
    let map = fst::Map::new(bytes).unwrap();
    let lev = Levenshtein::new("qeu", 2).unwrap();
    let mut stream = map.search(lev).into_stream();
    while let Some((k, v)) = stream.next() {
        let w = std::str::from_utf8(k).unwrap();
        println!("{} - {}", w, v);
    }
}
