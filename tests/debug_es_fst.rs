use fst::{IntoStreamer, Streamer};
use fst::automaton::Levenshtein;

#[test]
fn test_fst() {
    let bytes = std::fs::read("data/dictionaries/es.fst").unwrap();
    let map = fst::Map::new(bytes[4..].to_vec()).unwrap();
    let lev = Levenshtein::new("qeu", 2).unwrap();
    let mut stream = map.search(lev).into_stream();
    let mut count = 0;
    while let Some((k, v)) = stream.next() {
        let w = std::str::from_utf8(k).unwrap();
        println!("FOUND: {} - {}", w, v);
        count += 1;
    }
    println!("Total results: {}", count);
}
