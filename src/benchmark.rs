//! Benchmark suite for TypeFix
//!
//! Uses criterion for statistical benchmarking.
//!
//! Run benchmarks with: `cargo bench`

use std::sync::Arc;
use std::time::{Duration, Instant};

use typefix::core::{Trie, CharBuffer};
use typefix::correction::{CorrectionEngine, StaticErrorMap, DamerauLevenshtein};
use typefix::language::{LanguageDetector, StopwordsTrie};
use typefix::correction::engine::EngineConfig;
use typefix::language::detector::DetectorConfig;

use crate::memory::{get_memory_usage, MemoryStats, MemoryTracker, MemoryBenchmarkResult};

/// Benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub total_time_ms: f64,
    pub avg_time_ns: f64,
    pub ops_per_sec: f64,
}

impl BenchmarkResult {
    pub fn new(name: &str, iterations: usize, duration: Duration) -> Self {
        let total_time_ms = duration.as_secs_f64() * 1000.0;
        let total_ns = duration.as_nanos() as f64;
        let avg_time_ns = total_ns / iterations as f64;
        let ops_per_sec = (iterations as f64 * 1000.0) / total_time_ms;

        Self {
            name: name.to_string(),
            iterations,
            total_time_ms,
            avg_time_ns,
            ops_per_sec,
        }
    }

    /// Check if benchmark meets latency requirement (< 1ms = 1,000,000 ns)
    pub fn meets_latency_requirement(&self, max_ns: u64) -> bool {
        self.avg_time_ns < max_ns as f64
    }

    /// Check if benchmark meets throughput requirement
    pub fn meets_throughput_requirement(&self, min_ops_per_sec: f64) -> bool {
        self.ops_per_sec >= min_ops_per_sec
    }

    pub fn summary(&self) -> String {
        format!(
            "{}: {} iterations in {:.2}ms ({:.2}ns/op, {:.0} ops/sec)",
            self.name, self.iterations, self.total_time_ms, self.avg_time_ns, self.ops_per_sec
        )
    }
}

/// Memory benchmark result
pub struct MemoryBenchmark {
    pub name: String,
    pub initial_mb: f64,
    pub final_mb: f64,
    pub peak_mb: f64,
    pub under_limit: bool,
    pub limit_mb: f64,
}

impl MemoryBenchmark {
    pub fn new(name: &str, result: &MemoryBenchmarkResult, limit_mb: f64) -> Self {
        Self {
            name: name.to_string(),
            initial_mb: result.initial_mb,
            final_mb: result.final_mb,
            peak_mb: result.peak_mb,
            under_limit: result.under_limit,
            limit_mb,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{}: initial={:.2}MB, peak={:.2}MB, under_{:.0}MB={}",
            self.name, self.initial_mb, self.peak_mb, self.limit_mb, self.under_limit
        )
    }
}

// =============================================================================
// Trie Benchmarks
// =============================================================================

/// Benchmark Trie insertion
pub fn benchmark_trie_insert(word_count: usize) -> BenchmarkResult {
    let name = "trie_insert";
    let iterations = 10;
    let words = generate_test_words(word_count);

    let mut total_time = Duration::ZERO;

    for _ in 0..iterations {
        let mut trie = Trie::new();
        let start = Instant::now();

        for word in &words {
            trie.insert(word, 1);
        }

        total_time += start.elapsed();
    }

    let result = BenchmarkResult::new(
        &format!("{}_{}", name, word_count),
        word_count,
        total_time / iterations,
    );

    println!("{}", result.summary());
    result
}

/// Benchmark Trie search
pub fn benchmark_trie_search(word_count: usize, search_iterations: usize) -> BenchmarkResult {
    let name = "trie_search";
    let iterations = 10;
    let words = generate_test_words(word_count);
    let trie = build_trie(&words);
    let search_words: Vec<_> = words.iter().take(search_iterations).cloned().collect();

    let mut total_time = Duration::ZERO;

    for _ in 0..iterations {
        let start = Instant::now();

        for word in &search_words {
            let _ = trie.search(word);
        }

        total_time += start.elapsed();
    }

    let result = BenchmarkResult::new(
        &format!("{}_{}x{}", name, word_count, search_iterations),
        search_iterations,
        total_time / iterations,
    );

    println!("{}", result.summary());
    result
}

/// Benchmark Trie prefix search
pub fn benchmark_trie_prefix(prefix: &str, expected_results: usize) -> BenchmarkResult {
    let iterations = 10000;
    let words = generate_test_words(50000);
    let trie = build_trie(&words);

    let start = Instant::now();

    for _ in 0..iterations {
        let _ = trie.get_all_with_prefix(prefix, 10);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (iterations as f64 * 1000.0) / elapsed.as_secs_f64();

    BenchmarkResult {
        name: format!("trie_prefix_{}", prefix),
        iterations,
        total_time_ms: elapsed.as_secs_f64() * 1000.0,
        avg_time_ns: elapsed.as_nanos() as f64 / iterations as f64,
        ops_per_sec,
    }
}

// =============================================================================
// Correction Benchmarks
// =============================================================================

/// Benchmark single correction latency
pub fn benchmark_correction_latency() -> BenchmarkResult {
    let iterations = 10000;
    let engine = build_correction_engine();

    let typos = vec!["qeu", "teh", "adn", "hte", "wrok", " recive"];

    let start = Instant::now();

    for i in 0..iterations {
        let typo = &typos[i % typos.len()];
        let _ = engine.correct(typo);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (iterations as f64 * 1000.0) / elapsed.as_secs_f64();

    let result = BenchmarkResult {
        name: "correction_latency".to_string(),
        iterations,
        total_time_ms: elapsed.as_secs_f64() * 1000.0,
        avg_time_ns: elapsed.as_nanos() as f64 / iterations as f64,
        ops_per_sec,
    };

    println!("{}", result.summary());
    result
}

/// Benchmark throughput (corrections per second)
pub fn benchmark_throughput(duration_ms: u64) -> BenchmarkResult {
    let engine = Arc::new(build_correction_engine());
    let typos = vec!["qeu", "teh", "adn", "hte", "wrok"];

    let duration = Duration::from_millis(duration_ms);
    let start = Instant::now();
    let mut count = 0usize;

    while start.elapsed() < duration {
        let typo = &typos[count % typos.len()];
        let _ = engine.correct(typo);
        count += 1;
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (count as f64 * 1000.0) / elapsed.as_secs_f64();

    let result = BenchmarkResult {
        name: format!("throughput_{}ms", duration_ms),
        iterations: count,
        total_time_ms: elapsed.as_secs_f64() * 1000.0,
        avg_time_ns: elapsed.as_nanos() as f64 / count as f64,
        ops_per_sec,
    };

    println!("{}", result.summary());
    result
}

/// Benchmark Damerau-Levenshtein
pub fn benchmark_damerau() -> BenchmarkResult {
    let iterations = 100000;
    let calc = &mut DamerauLevenshtein::new();
    let test_cases = vec![
        ("qeu", "que"),
        ("teh", "the"),
        ("adn", "and"),
        ("hello", "helo"),
        ("test", "test"),
    ];

    let start = Instant::now();

    for i in 0..iterations {
        let (s1, s2) = &test_cases[i % test_cases.len()];
        let _ = calc.distance(s1, s2, 2);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (iterations as f64 * 1000.0) / elapsed.as_secs_f64();

    BenchmarkResult {
        name: "damerau_distance".to_string(),
        iterations,
        total_time_ms: elapsed.as_secs_f64() * 1000.0,
        avg_time_ns: elapsed.as_nanos() as f64 / iterations as f64,
        ops_per_sec,
    }
}

// =============================================================================
// Memory Benchmarks
// =============================================================================

/// Benchmark memory usage of engine
pub fn benchmark_engine_memory() -> MemoryBenchmark {
    let initial = get_memory_usage();

    // Create engine with dictionaries
    let engine = build_correction_engine();

    // Take samples during operation
    let mut tracker = MemoryTracker::new();
    tracker.start();

    // Simulate corrections
    let typos = vec!["qeu", "teh", "adn"];
    for _ in 0..1000 {
        for typo in &typos {
            let _ = engine.correct(typo);
        }
        tracker.sample();
    }

    let final_stats = get_memory_usage();
    let peak = tracker.peak_memory();

    let result = MemoryBenchmarkResult::new(initial, final_stats, peak.clone(), 10.0);
    let benchmark = MemoryBenchmark::new("engine_memory", &result, 10.0);

    println!("{}", benchmark.summary());
    benchmark
}

/// Benchmark dictionary loading memory
pub fn benchmark_dictionary_memory() -> MemoryBenchmark {
    let initial = get_memory_usage();

    // Build large dictionary
    let mut trie = Trie::new();
    for i in 0..50000 {
        let word = format!("word{:06}", i);
        trie.insert(&word, 1000 - i as u64);
    }

    let final_stats = get_memory_usage();
    let increase = (final_stats.rss as f64 - initial.rss as f64) / (1024.0 * 1024.0);

    let under_limit = increase < 10.0;

    println!(
        "Dictionary memory: initial={:.2}MB, final={:.2}MB, increase={:.2}MB, under_10MB={}",
        initial.as_mb(),
        final_stats.as_mb(),
        increase,
        under_limit
    );

    MemoryBenchmark {
        name: "dictionary_memory".to_string(),
        initial_mb: initial.as_mb(),
        final_mb: final_stats.as_mb(),
        peak_mb: final_stats.as_mb(),
        under_limit,
        limit_mb: 10.0,
    }
}

// =============================================================================
// Language Detection Benchmarks
// =============================================================================

/// Benchmark language detection
pub fn benchmark_language_detection(word_count: usize) -> BenchmarkResult {
    let iterations = 10000;
    let detector = build_language_detector();
    let words = vec![
        "el", "la", "de", "que", "es", "y", "en", "un", "por", "con",
        "the", "a", "of", "is", "and", "to", "in", "that", "it", "for",
    ];

    let start = Instant::now();

    for i in 0..iterations {
        let word = &words[i % words.len()];
        let _ = detector.process_word(word);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (iterations as f64 * 1000.0) / elapsed.as_secs_f64();

    let result = BenchmarkResult {
        name: format!("language_detection_{}", word_count),
        iterations,
        total_time_ms: elapsed.as_secs_f64() * 1000.0,
        avg_time_ns: elapsed.as_nanos() as f64 / iterations as f64,
        ops_per_sec,
    };

    println!("{}", result.summary());
    result
}

// =============================================================================
// Helper Functions
// =============================================================================

fn generate_test_words(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| format!("word{:06}", i))
        .collect()
}

fn build_trie(words: &[String]) -> Trie {
    let mut trie = Trie::new();
    for (i, word) in words.iter().enumerate() {
        trie.insert(word, (1000 - i as u64).max(1));
    }
    trie
}

fn build_correction_engine() -> CorrectionEngine {
    let mut engine = CorrectionEngine::new(EngineConfig::default());

    // Add English dictionary
    let mut en_dict = Trie::new();
    for (word, freq) in &[
        ("the", 1000000),
        ("que", 900000),
        ("and", 800000),
        ("hello", 500000),
        ("world", 400000),
        ("test", 300000),
        ("receive", 200000),
        ("work", 150000),
    ] {
        en_dict.insert(word, *freq);
    }
    engine.add_dictionary("en", Arc::new(en_dict));

    // Add error map
    let mut error_map = StaticErrorMap::new("en");
    error_map.insert_static("qeu", "que");
    error_map.insert_static("teh", "the");
    error_map.insert_static("adn", "and");
    error_map.insert_static("hte", "the");
    error_map.insert_static("wrok", "work");
    error_map.insert_static(" recive", "receive");
    engine.add_error_map(Arc::new(error_map), "en");

    engine
}

fn build_language_detector() -> LanguageDetector {
    let config = DetectorConfig {
        window_size: 5,
        confidence_threshold: 0.85,
        hysteresis_zone: 0.10,
        min_words_before_switch: 5,
    };
    let mut detector = LanguageDetector::new(config);

    // Add Spanish stopwords
    let mut es_stopwords = StopwordsTrie::new();
    for w in &["el", "la", "de", "que", "es", "y", "en", "un", "por", "con"] {
        es_stopwords.insert(w);
    }
    detector.add_language("es", Arc::new(es_stopwords));

    // Add English stopwords
    let mut en_stopwords = StopwordsTrie::new();
    for w in &["the", "a", "of", "is", "and", "to", "in", "that", "it", "for"] {
        en_stopwords.insert(w);
    }
    detector.add_language("en", Arc::new(en_stopwords));

    detector
}

// =============================================================================
// Full Benchmark Suite
// =============================================================================

/// Run all benchmarks and return results
pub fn run_full_benchmark_suite() -> Vec<String> {
    let mut results = Vec::new();

    println!("\n=== TypeFix Benchmark Suite ===\n");

    // Memory benchmarks
    println!("\n--- Memory Benchmarks ---");
    let mem_engine = benchmark_engine_memory();
    let mem_dict = benchmark_dictionary_memory();
    results.push(mem_engine.summary());
    results.push(mem_dict.summary());

    // Trie benchmarks
    println!("\n--- Trie Benchmarks ---");
    let trie_insert = benchmark_trie_insert(50000);
    let trie_search = benchmark_trie_search(50000, 10000);
    let trie_prefix = benchmark_trie_prefix("word00", 100);
    results.push(trie_insert.summary());
    results.push(trie_search.summary());
    results.push(trie_prefix.summary());

    // Correction benchmarks
    println!("\n--- Correction Benchmarks ---");
    let correction_latency = benchmark_correction_latency();
    let throughput = benchmark_throughput(1000); // 1 second
    let damerau = benchmark_damerau();
    results.push(correction_latency.summary());
    results.push(throughput.summary());
    results.push(damerau.summary());

    // Language detection benchmarks
    println!("\n--- Language Detection Benchmarks ---");
    let lang_detection = benchmark_language_detection(1000);
    results.push(lang_detection.summary());

    // Verify requirements
    println!("\n=== Requirements Verification ===");
    let latency_ok = correction_latency.meets_latency_requirement(1_000_000); // 1ms
    let throughput_ok = throughput.meets_throughput_requirement(10000.0); // 10K ops/sec
    let memory_ok = mem_engine.under_limit && mem_dict.under_limit;

    println!("Latency < 1ms: {}", latency_ok);
    println!("Throughput >= 10K/sec: {}", throughput_ok);
    println!("Memory < 10MB: {}", memory_ok);

    if latency_ok && throughput_ok && memory_ok {
        println!("\n✅ ALL REQUIREMENTS MET");
    } else {
        println!("\n⚠️ SOME REQUIREMENTS NOT MET");
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_result() {
        let result = BenchmarkResult::new("test", 1000, Duration::from_millis(100));
        assert_eq!(result.iterations, 1000);
        assert_eq!(result.total_time_ms, 100.0);
        assert!(result.avg_time_ns > 0.0);
        assert!(result.ops_per_sec > 0.0);
    }

    #[test]
    fn test_latency_requirement() {
        let result = BenchmarkResult::new("test", 1000, Duration::from_millis(100));
        // 100ms for 1000 ops = 100,000ns per op = 0.1ms
        assert!(result.meets_latency_requirement(1_000_000)); // 1ms limit
    }

    #[test]
    fn test_trie_insert_small() {
        let result = benchmark_trie_insert(1000);
        assert!(result.avg_time_ns < 100_000_000.0); // Should be fast
    }
}
