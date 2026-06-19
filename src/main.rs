//! TypeFix - CLI Entry Point
//!
//! Usage:
//!   typefix              # Start as daemon
//!   typefix -c <file>    # Start with custom config
//!   typefix repl         # Interactive REPL mode
//!   typefix correct <word> # Correct a single word
//!   typefix bench        # Run benchmarks

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use typefix::hooks::platform::{KeyEvent, SpecialKey};
use typefix::pipeline::TypeFixPipeline;

fn main() -> Result<()> {
    // Parse CLI arguments
    let matches = clap::Command::new("typefix")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Hyper-lightweight typo correction and language detection engine")
        .subcommand(clap::Command::new("repl").about("Start interactive REPL mode"))
        .subcommand(
            clap::Command::new("correct")
                .about("Correct a single word")
                .arg(clap::Arg::new("word").required(true)),
        )
        .subcommand(clap::Command::new("bench").about("Run performance benchmarks"))
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .value_name("PATH")
                .help("Configuration file path")
                .default_value("config.json"),
        )
        .arg(
            clap::Arg::new("data-path")
                .short('d')
                .long("data-path")
                .value_name("PATH")
                .help("Data directory path")
                .default_value("data"),
        )
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::SetTrue)
                .help("Enable verbose logging"),
        )
        .get_matches();

    // Initialize logging
    let log_level = if matches.get_flag("verbose") {
        "debug"
    } else {
        "info"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // Handle subcommands
    match matches.subcommand_name() {
        Some("repl") => run_repl()?,
        Some("correct") => {
            if let Some(correct_matches) = matches.subcommand_matches("correct") {
                if let Some(word) = correct_matches.get_one::<String>("word") {
                    correct_word(word)?;
                }
            }
        }
        Some("bench") => run_benchmarks()?,
        _ => run_daemon(matches.clone())?,
    }

    Ok(())
}

fn run_daemon(matches: clap::ArgMatches) -> Result<()> {
    let config_path = matches
        .get_one::<String>("config")
        .ok_or_else(|| anyhow::anyhow!("Missing --config argument"))?;
    let data_path: PathBuf = matches
        .get_one::<String>("data-path")
        .ok_or_else(|| anyhow::anyhow!("Missing --data-path argument"))?
        .into();

    tracing::info!("Loading configuration from {}", config_path);

    let config =
        typefix::core::config::Config::from_file(config_path).context("Failed to load config")?;
    let config = typefix::core::config::Config {
        data_path,
        ..config
    };

    typefix::init(&config)?;

    if matches!(
        config.hooks.mode,
        typefix::core::config::HookMode::Disabled
    ) {
        tracing::info!("Hook mode disabled - running in API-only mode");
        tracing::info!("Press Enter to exit");
        use std::io::{self, Read};
        let _ = io::stdin().read(&mut [0u8]);
        tracing::info!("Shutting down...");
        return Ok(());
    }

    tracing::info!("Initializing Windows keyboard hook...");

    let hook_config = typefix::hooks::platform::HookConfig {
        enabled: config.hooks.keyboard_enabled,
        log_keystrokes: config.hooks.log_keystrokes,
        mode: match config.hooks.mode {
            typefix::core::config::HookMode::System => {
                typefix::hooks::platform::HookMode::System
            }
            typefix::core::config::HookMode::Application => {
                typefix::hooks::platform::HookMode::Application
            }
            typefix::core::config::HookMode::Disabled => {
                typefix::hooks::platform::HookMode::Disabled
            }
        },
    };

    let hook = typefix::hooks::platform::create_hook(hook_config)
        .context("Failed to create keyboard hook")?;

    if let Err(e) = hook.start() {
        tracing::error!("Failed to start keyboard hook: {}", e);
        tracing::info!("Press Enter to exit");
        use std::io::{self, Read};
        let _ = io::stdin().read(&mut [0u8]);
        tracing::info!("Shutting down...");
        return Ok(());
    }

    tracing::info!("TypeFix started successfully - monitoring keyboard input");

    let pipeline = TypeFixPipeline::new(typefix::pipeline::PipelineConfig::default());
    {
        let state_arc = typefix::get_state();
        let state = state_arc.read();
        for (lang, dict) in &state.dictionaries {
            pipeline.add_dictionary(lang, std::sync::Arc::clone(dict));
        }
        for (lang, sw) in &state.stopwords {
            pipeline.add_stopwords(lang, std::sync::Arc::clone(sw));
        }
        for (lang, em) in &state.error_maps {
            pipeline.add_error_map(lang, std::sync::Arc::clone(em));
        }
        pipeline.set_language(&state.active_language);
    }

    let receiver = hook.receiver();
    loop {
        let event = match receiver.recv() {
            Ok(event) => event,
            Err(_) => {
                tracing::info!("Hook receiver disconnected, shutting down...");
                break;
            }
        };

        match event.event {
            KeyEvent::Char(ch) => {
                if let Some(result) = pipeline.push(ch) {
                        if let Some(corrected) = result.corrected {
                            tracing::info!(
                                "Correction: '{}' -> '{}'",
                                result.original,
                                corrected
                            );
                            let backspaces = result.original.chars().count() + 1;
                            for _ in 0..backspaces {
                                if let Err(e) = hook.send_text("\x08") {
                                    tracing::error!("Failed to send backspace: {}", e);
                                }
                            }
                            let corrected_with_delimiter = format!("{}{}", corrected, ch);
                            if let Err(e) = hook.send_text(&corrected_with_delimiter) {
                                tracing::error!("Failed to send correction text: {}", e);
                            }
                        }
                }
            }
            KeyEvent::Special(SpecialKey::Backspace) => {
                pipeline.clear();
            }
            KeyEvent::Special(
                SpecialKey::Enter | SpecialKey::Tab | SpecialKey::Escape,
            ) => {
                // Push a space to force delimiter extraction
                let _ = pipeline.push(' ');
            }
            KeyEvent::Special(_) | KeyEvent::Control(_) => {}
        }
    }

    tracing::info!("Shutting down...");
    Ok(())
}

fn run_repl() -> Result<()> {
    use typefix::{PipelineEvent, TypeFixPipeline};

    println!("\n╔══════════════════════════════════════╗");
    println!("║       TypeFix REPL Mode          ║");
    println!("╚══════════════════════════════════════╝");
    println!("Type text to see corrections. Press Ctrl+D to exit.\n");

    let config = typefix::core::config::Config::default();
    let _ = typefix::init(&config);
    let pipeline = TypeFixPipeline::new(typefix::pipeline::PipelineConfig::default());
    {
        let state_arc = typefix::get_state();
        let state = state_arc.read();
        for (lang, dict) in &state.dictionaries {
            pipeline.add_dictionary(lang, std::sync::Arc::clone(dict));
        }
        for (lang, sw) in &state.stopwords {
            pipeline.add_stopwords(lang, std::sync::Arc::clone(sw));
        }
        for (lang, em) in &state.error_maps {
            pipeline.add_error_map(lang, std::sync::Arc::clone(em));
        }
        pipeline.set_language(&state.active_language);
    }

    // Subscribe to events
    pipeline.on_event(|event| match event {
        PipelineEvent::WordExtracted { word } => {
            println!("  [word] {}", word);
        }
        PipelineEvent::WordCorrected {
            original,
            corrected,
        } => {
            println!("  [fix]  {} → {}", original, corrected);
        }
        PipelineEvent::LanguageDetected {
            language,
            confidence,
        } => {
            println!(
                "  [lang] {} ({:.0}% confidence)",
                language,
                confidence * 100.0
            );
        }
        PipelineEvent::BufferOverflow { word } => {
            println!("  [warn] Buffer overflow: {}", word);
        }
    });

    loop {
        print!("> ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(0) => break, // EOF
            Ok(_) => {
                for ch in input.chars() {
                    let _ = pipeline.push(ch);
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }

    println!("\nGoodbye!");
    Ok(())
}

fn correct_word(word: &str) -> Result<()> {
    use std::sync::Arc;
    use typefix::core::Trie;
    use typefix::{CorrectionEngine, EngineConfig};

    let engine = CorrectionEngine::new(EngineConfig::default());

    // Add test dictionary
    let mut trie = Trie::new();
    trie.insert("hello", 1000);
    trie.insert("world", 900);
    trie.insert("the", 800);
    trie.insert("que", 700);
    trie.insert("test", 600);
    engine.add_dictionary("en", Arc::new(trie));

    let result = engine.correct(word);
    if let Some(corrected) = result.corrected {
        println!("{}", corrected);
    } else {
        println!("{}", word);
    }

    Ok(())
}

fn run_benchmarks() -> Result<()> {
    println!("\n╔══════════════════════════════════════╗");
    println!("║       TypeFix Benchmarks        ║");
    println!("╚══════════════════════════════════════╝\n");

    // Run stress tests
    println!("Running stress tests...\n");

    use std::time::Instant;
    use typefix::core::Trie;
    use typefix::TypeFixPipeline;

    // Benchmark 1: Pipeline throughput
    let pipeline = TypeFixPipeline::simple();
    let test_text =
        "this is a test of the typo correction engine with some common typos like teh and qeu";

    let start = Instant::now();
    let mut words_processed = 0;

    for ch in test_text.chars() {
        if pipeline.push(ch).is_some() {
            words_processed += 1;
        }
    }

    let elapsed = start.elapsed();
    let chars_per_sec = (test_text.len() as f64 * 1000.0) / elapsed.as_millis() as f64;

    println!("Pipeline throughput:");
    println!("  - Chars/sec: {:.0}", chars_per_sec);
    println!("  - Words processed: {}", words_processed);
    println!("  - Time: {:.2}ms\n", elapsed.as_secs_f64() * 1000.0);

    // Benchmark 2: Dictionary operations
    let mut trie = Trie::new();
    let word_count = 50_000;

    let start = Instant::now();
    for i in 0..word_count {
        trie.insert(&format!("word{:06}", i), i as u64);
    }
    let insert_time = start.elapsed();

    let start = Instant::now();
    for i in 0..1000 {
        let _ = trie.search(&format!("word{:06}", i * 7 % word_count));
    }
    let search_time = start.elapsed();

    println!("Dictionary operations ({} words):", word_count);
    println!(
        "  - Insert time: {:.2}ms",
        insert_time.as_secs_f64() * 1000.0
    );
    println!(
        "  - Search (1K ops): {:.2}ms",
        search_time.as_secs_f64() * 1000.0
    );
    println!(
        "  - Average search: {:.2}µs\n",
        (search_time.as_nanos() as f64 / 1000.0) / 1000.0
    );

    println!("Benchmarks complete!");
    Ok(())
}
