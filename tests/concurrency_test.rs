//! Concurrency stress tests for Section 7 (Zero Shared Global State).
//!
//! These tests exercise the read-mostly paths of the engine from many threads at
//! once to catch data races, deadlocks, and logical hazards. They are intended
//! to be run with `cargo test --release --test concurrency_test` and can also
//! be enabled under ThreadSanitizer (`RUSTFLAGS="-Z sanitizer=thread"`) on
//! nightly to detect races that the borrow checker would otherwise hide.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use typefix::core::Trie;
use typefix::correction::engine::EngineConfig;
use typefix::correction::CorrectionEngine;
use typefix::correction::StaticErrorMap;
use typefix::language::detector::{DetectorConfig, LanguageDetector};
use typefix::language::StopwordsTrie;
use typefix::pipeline::{PipelineConfig, TypeFixPipeline};

/// Build a single CorrectionEngine preloaded with a small English dictionary
/// and a handful of static typo corrections. The engine is wrapped in an
/// `Arc` so the same instance can be shared across many threads.
fn shared_correction_engine() -> Arc<CorrectionEngine> {
    let mut trie = Trie::new();
    for (w, freq) in [
        ("hello", 1000u64),
        ("world", 900),
        ("the", 10000),
        ("and", 9000),
        ("people", 4000),
        ("which", 3000),
        ("would", 2500),
    ] {
        trie.insert(w, freq);
    }

    let errors = {
        let e = StaticErrorMap::new("en");
        e.insert_static("qeu", "que");
        e.insert_static("teh", "the");
        e.insert_static("wnat", "want");
        e
    };

    let engine = CorrectionEngine::new(EngineConfig {
        max_edit_distance: 2,
        max_candidates: 5,
        min_word_length: 2,
        case_sensitive: false,
    });
    engine.add_dictionary("en", Arc::new(trie));
    engine.add_error_map(Arc::new(errors), "en");

    let detector = Arc::new(LanguageDetector::new(DetectorConfig {
        min_words_before_switch: 1,
        ..DetectorConfig::default()
    }));
    detector.set_language("en");

    let engine = engine.with_detector(detector);

    Arc::new(engine)
}

/// Read-only stress: N threads call `correct` repeatedly. The engine is
/// reference-counted (Arc<CorrectionEngine>) and uses internal RwLock, so
/// readers must not contend with each other.
#[test]
fn test_concurrent_reads_correction_engine() {
    const THREADS: usize = 8;
    const ITERATIONS: usize = 2_000;

    let engine = shared_correction_engine();
    // Use a static array so the `move ||` closure can borrow from it on every
    // iteration without re-cloning.
    let inputs: [&str; 8] = [
        "hello", "teh", "qeu", "wnat", "world", "people", "helo", "worrld",
    ];

    let start = Instant::now();
    let mut handles = Vec::with_capacity(THREADS);
    for tid in 0..THREADS {
        let engine = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            let mut local_hits = 0u64;
            for i in 0..ITERATIONS {
                let word = inputs[(tid + i) % inputs.len()];
                let result = engine.correct(word);
                if result.corrected.is_some() {
                    local_hits += 1;
                }
            }
            local_hits
        }));
    }
    let total_hits: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let elapsed = start.elapsed();

    // Loose sanity: a few hundred corrections per thread should resolve
    // through the static map, but we never assert exact counts because
    // load order is non-deterministic.
    assert!(
        total_hits > 0,
        "no corrections produced under concurrent read"
    );
    println!(
        "concurrent_reads: {} threads x {} iters in {:?} ({} corrections)",
        THREADS, ITERATIONS, elapsed, total_hits
    );
}

/// Mixed read/write stress: while reader threads hammer `correct`, writer
/// threads call `mark_correct`/`mark_incorrect` to mutate the static error
/// map. The map uses `Arc<RwLock<...>>` internally so reads and writes must
/// serialise cleanly.
#[test]
fn test_concurrent_reads_writes_static_map() {
    const READERS: usize = 6;
    const WRITERS: usize = 2;
    const ITERATIONS: usize = 1_500;

    let engine = shared_correction_engine();
    let inputs: [&str; 6] = ["qeu", "teh", "wnat", "hello", "world", "helo"];

    let mut handles = Vec::with_capacity(READERS + WRITERS);
    for tid in 0..READERS {
        let engine = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            for i in 0..ITERATIONS {
                let word = inputs[(tid + i) % inputs.len()];
                let _ = engine.correct(word);
            }
        }));
    }
    for tid in 0..WRITERS {
        let engine = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            for i in 0..ITERATIONS {
                let typo = format!("typo_{}_{}", tid, i);
                let correction = format!("correct_{}_{}", tid, i);
                engine.mark_correct(&typo, &correction);
                if i % 17 == 0 {
                    engine.mark_incorrect(&typo);
                }
            }
        }));
    }
    for h in handles {
        h.join().expect("thread panicked");
    }
    // We made it through without a panic or deadlock.
}

/// Pipeline stress: many threads push characters into the SAME
/// `Arc<TypeFixPipeline>`. This is the hottest read path the engine exposes -
/// buffer + correction + event callbacks - and must hold up under contention.
#[test]
fn test_concurrent_pipeline_push() {
    const THREADS: usize = 6;
    const CHARS_PER_THREAD: usize = 500;

    let pipeline = Arc::new(TypeFixPipeline::simple());

    // Attach a single shared event collector so we also stress the
    // pipeline's internal RwLock<Vec<Box<dyn Fn(...)>>>.
    let event_count = Arc::new(RwLock::new(0u64));
    let counter = Arc::clone(&event_count);
    pipeline.on_event(move |_event| {
        let mut g = counter.write();
        *g += 1;
    });

    let mut handles = Vec::with_capacity(THREADS);
    for _ in 0..THREADS {
        let pipeline = Arc::clone(&pipeline);
        handles.push(thread::spawn(move || {
            for i in 0..CHARS_PER_THREAD {
                let ch = match i % 5 {
                    0 => 'h',
                    1 => 'e',
                    2 => 'l',
                    3 => 'o',
                    _ => ' ',
                };
                let _ = pipeline.push(ch);
            }
        }));
    }
    for h in handles {
        h.join().expect("thread panicked");
    }

    // The exact number depends on tokenisation order, but at least some
    // events must have been delivered to the shared collector.
    let total = *event_count.read();
    assert!(
        total > 0,
        "no pipeline events delivered under concurrent push"
    );
}

/// Detector stress: many threads feed words into the same
/// `Arc<LanguageDetector>` while another writer flips the current language.
/// This is the trickiest case because the detector holds FOUR independent
/// `RwLock`s that the implementation grabs in a non-trivial order.
#[test]
fn test_concurrent_detector_updates() {
    const FEEDERS: usize = 6;
    const WORDS_PER_FEEDER: usize = 2_000;

    let mut es_stop = StopwordsTrie::new();
    for w in ["el", "la", "de", "que", "es", "y", "en", "un", "una", "los"] {
        es_stop.insert(w);
    }

    let detector = Arc::new(LanguageDetector::new(DetectorConfig {
        min_words_before_switch: 4,
        ..DetectorConfig::default()
    }));
    detector.add_language("es", Arc::new(es_stop));
    detector.set_language("en");

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut handles = Vec::with_capacity(FEEDERS + 1);

    for tid in 0..FEEDERS {
        let detector = Arc::clone(&detector);
        let stop = Arc::clone(&stop);
        handles.push(thread::spawn(move || {
            for i in 0..WORDS_PER_FEEDER {
                if stop.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let word = if (tid + i) % 3 == 0 { "el" } else { "the" };
                let _ = detector.process_word(word);
            }
        }));
    }

    // Writer: every 1ms flip the language to exercise the current_language
    // write path alongside the readers.
    let detector_w = Arc::clone(&detector);
    let stop_w = Arc::clone(&stop);
    handles.push(thread::spawn(move || {
        for i in 0..200 {
            if i % 2 == 0 {
                detector_w.set_language("en");
            } else {
                detector_w.set_language("es");
            }
            thread::sleep(Duration::from_millis(1));
        }
        stop_w.store(true, std::sync::atomic::Ordering::Relaxed);
    }));

    for h in handles {
        h.join().expect("thread panicked");
    }
}

/// Compile-time check: CorrectionEngine and TypeFixPipeline must be Send + Sync
/// so they can be wrapped in `Arc` and shared across threads. This test
/// doesn't run any code; the assertion is on the type system.
#[test]
fn test_send_sync_compile_time() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CorrectionEngine>();
    assert_send_sync::<TypeFixPipeline>();
    assert_send_sync::<LanguageDetector>();
    assert_send_sync::<StaticErrorMap>();
    // Build a value to make sure the type actually exists at runtime.
    let _ = CorrectionEngine::new(EngineConfig::default());
    let _ = TypeFixPipeline::new(PipelineConfig::default());
    let _ = LanguageDetector::new(DetectorConfig::default());
    let _ = StaticErrorMap::new("en");
}
