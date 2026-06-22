#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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
    // Fail-fast on panic to prevent undefined behavior
    std::panic::set_hook(Box::new(|info| {
        eprintln!("FATAL PANIC: {}", info);
        std::process::abort();
    }));

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
        .subcommand(
            clap::Command::new("build-dict")
                .about("Compile JSON dictionary to binary FST format")
                .arg(clap::Arg::new("input").required(true).help("Input JSON file"))
                .arg(clap::Arg::new("output").required(true).help("Output FST file")),
        )
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
        Some("build-dict") => {
            if let Some(build_matches) = matches.subcommand_matches("build-dict") {
                run_build_dict(build_matches)?;
            }
        }
        _ => run_daemon(matches.clone())?,
    }

    Ok(())
}

fn run_daemon(matches: clap::ArgMatches) -> Result<()> {
    // If the user specified a config file via CLI, use it. Otherwise, use the OS native config dir.
    let (config, config_path) = if let Some(path) = matches.get_one::<String>("config") {
        let p = PathBuf::from(path);
        if p.exists() || path != "config.json" {
            (
                typefix::core::config::Config::from_file(path)
                    .with_context(|| format!("Failed to load config from {}", path))?,
                p,
            )
        } else {
            typefix::core::config::Config::load_or_default()
                .context("Failed to load or create default native config")?
        }
    } else {
        typefix::core::config::Config::load_or_default()
            .context("Failed to load or create default native config")?
    };

    tracing::info!("Loaded configuration from {}", config_path.display());

    let data_path: PathBuf = matches
        .get_one::<String>("data-path")
        .ok_or_else(|| anyhow::anyhow!("Missing --data-path argument"))?
        .into();

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

    std::thread::spawn(move || {
        let mut current_window_id = 0;

        loop {
            let event = match hook.receiver().recv() {
                Ok(event) => event,
                Err(_) => {
                    tracing::info!("Hook receiver disconnected, shutting down...");
                    break;
                }
            };

            if current_window_id != 0 && event.window_id != current_window_id {
                pipeline.clear();
            }
            current_window_id = event.window_id;

            match event.event {
                KeyEvent::Char(ch) => {
                    if let Some(result) = pipeline.push(ch) {
                        // Auto-Switch Language if a confident detection occurred
                        if let Some(detection) = &result.detected_language {
                            tracing::info!(
                                "Auto-Switching language to '{}' (confidence: {:.2})",
                                detection.language,
                                detection.confidence
                            );
                            pipeline.set_language(&detection.language);
                        }

                        if let Some(ref corrected) = result.corrected {
                            if !hook.is_window_active(current_window_id) {
                                tracing::warn!("Window focus changed! Aborting auto-correction to prevent injection into wrong window.");
                                pipeline.clear();
                                continue;
                            }

                            tracing::info!(
                                "Correction: '{}' -> '{}'",
                                result.original,
                                corrected
                            );
                            let backspaces = result.original.chars().count() + 1;
                            let corrected_with_delimiter = format!("{}{}", corrected, ch);
                            if let Err(e) = hook.send_correction_atomic(backspaces, &corrected_with_delimiter, current_window_id) {
                                tracing::error!("Failed to inject correction atomically: {}", e);
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
    });

    tracing::info!("Starting System Tray...");

    let event_loop = tao::event_loop::EventLoop::new();
    let tray_menu = tray_icon::menu::Menu::new();
    let quit_i = tray_icon::menu::MenuItem::new("Quit", true, None);
    let _ = tray_menu.append(&quit_i);

    let (width, height) = (32, 32);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                rgba.extend_from_slice(&[255, 255, 255, 255]);
            } else {
                rgba.extend_from_slice(&[50, 100, 200, 255]);
            }
        }
    }
    let icon = tray_icon::Icon::from_rgba(rgba, width, height).unwrap();

    let _tray_icon = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("TypeFix")
        .with_icon(icon)
        .build()
        .unwrap();

    let menu_channel = tray_icon::menu::MenuEvent::receiver();

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = tao::event_loop::ControlFlow::Wait;

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_i.id() {
                tracing::info!("Quit requested via tray menu.");
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
        }
    });
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
    use typefix::{CorrectionEngine, EngineConfig};
    let (config, _) = typefix::core::config::Config::load_or_default()?;
    typefix::init(&config)?;
    let state = typefix::get_state();
    let s = state.read();

    let engine_config = EngineConfig {
        max_edit_distance: config.correction.max_edit_distance,
        max_candidates: config.correction.max_corrections,
        min_word_length: config.correction.min_word_length,
        case_sensitive: config.correction.case_sensitive,
        enforce_accents: config.correction.enforce_accents,
    };
    let engine = CorrectionEngine::new(engine_config);
    
    for (lang, dict) in &s.dictionaries {
        engine.add_dictionary(lang, dict.clone());
    }
    
    for (lang, em) in &s.error_maps {
        engine.add_error_map(em.clone(), lang);
}

    engine.set_language(&s.active_language);

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
    use typefix::core::Dict;
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
    let word_count = 50_000;
    
    let mut entries: Vec<(String, u64)> = (0..word_count)
        .map(|i| (format!("word{:06}", i), i as u64))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let start = Instant::now();
    let mut builder = fst::MapBuilder::memory();
    for (w, f) in &entries {
        builder.insert(w.as_bytes(), *f).unwrap();
    }
    let dict = Dict::from_bytes(typefix::core::dict::wrap_fst_bytes(&builder.into_inner().unwrap())).unwrap();
    let insert_time = start.elapsed();

    let start = Instant::now();
    for i in 0..1000 {
        let _ = dict.search(&format!("word{:06}", i * 7 % word_count));
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

fn run_build_dict(matches: &clap::ArgMatches) -> Result<()> {
    let input = matches.get_one::<String>("input").unwrap();
    let output = matches.get_one::<String>("output").unwrap();
    
    let input_path = std::path::Path::new(input);
    let output_path = std::path::Path::new(output);

    println!("Compiling JSON dictionary at {} to FST at {}", input, output);
    typefix::core::Dict::compile_json_to_fst(input_path, output_path)?;
    println!("Compilation successful.");

    Ok(())
}
