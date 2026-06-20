use std::fs;
use fst::{IntoStreamer, Streamer};
use fst::automaton::Levenshtein;

fn main() {
    let bytes = fs::read("data/dictionaries/es.fst").unwrap();
    let map = fst::Map::new(bytes).unwrap();
    let lev = Levenshtein::new("qeu", 2).unwrap();
    let mut stream = map.search(lev).into_stream();
    let mut count = 0;
    while let Some((k, v)) = stream.next() {
        let w = std::str::from_utf8(k).unwrap();
        println!("{} - {}", w, v);
        count += 1;
    }
    println!("Total results: {}", count);
}
