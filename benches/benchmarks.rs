//! Benchmark suite for TypeFix

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    // Dict benchmarks
    c.bench_function("dict_build", |b| {
        use typefix::core::Dict;
        b.iter(|| {
            let mut builder = fst::MapBuilder::memory();
            builder.insert("benchmark", 1000).unwrap();
            let _dict = Dict::from_bytes(builder.into_inner().unwrap()).unwrap();
        });
    });

    c.bench_function("dict_search", |b| {
        use typefix::core::Dict;
        let mut builder = fst::MapBuilder::memory();
        builder.insert("hello", 1000).unwrap();
        builder.insert("world", 800).unwrap();
        let dict = Dict::from_bytes(builder.into_inner().unwrap()).unwrap();
        b.iter(|| dict.search(black_box("hello")));
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
        let mut calc = DamerauLevenshtein::new();
        b.iter(|| calc.distance(black_box("qeu"), black_box("que"), black_box(1)));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(1000);
    targets = criterion_benchmark
}
criterion_main!(benches);
