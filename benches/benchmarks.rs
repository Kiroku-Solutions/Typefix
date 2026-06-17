//! Benchmark suite for TypeFix

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    // Trie benchmarks
    c.bench_function("trie_insert", |b| {
        use typefix::Trie;
        let mut trie = Trie::new();
        b.iter(|| {
            let mut t = trie.clone();
            t.insert(black_box("benchmark"), black_box(1000));
        });
    });

    c.bench_function("trie_search", |b| {
        use typefix::Trie;
        let mut trie = Trie::new();
        trie.insert("hello", 1000);
        trie.insert("world", 800);
        b.iter(|| trie.search(black_box("hello")));
    });

    // Buffer benchmarks
    c.bench_function("buffer_push", |b| {
        use typefix::CharBuffer;
        let buffer = CharBuffer::new();
        b.iter(|| buffer.push(black_box('a')));
    });

    // Damerau-Levenshtein benchmarks
    c.bench_function("damerau_distance_one", |b| {
        use typefix::DamerauLevenshtein;
        let calc = DamerauLevenshtein::new();
        b.iter(|| calc.distance(black_box("qeu"), black_box("que"), black_box(1)));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(1000);
    targets = criterion_benchmark
}
criterion_main!(benches);
