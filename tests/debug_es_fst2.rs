#[test]
fn test_fst2() {
    let bytes = std::fs::read("t:/Kiroku/Kiroku/Kiroku-landing/public/data/dictionaries/es.fst").unwrap();
    let map = fst::Map::new(bytes).unwrap();
    let lev = fst::automaton::Levenshtein::new("vaina", 2).unwrap();
    let mut stream = map.search(lev).into_stream();
    while let Some((k, v)) = fst::Streamer::next(&mut stream) {
        let w = std::str::from_utf8(k).unwrap();
        println!("FOUND: {} - {}", w, v);
    }
}
