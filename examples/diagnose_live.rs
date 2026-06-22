/// Live diagnostic: starts the real daemon with verbose logging on every step
use std::sync::Arc;
use typefix::hooks::platform::{KeyEvent, SpecialKey};

fn main() {
    // Force debug logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let config = typefix::core::config::Config {
        data_path: std::path::PathBuf::from("data"),
        default_language: "en".to_string(),
        supported_languages: vec!["en".to_string(), "es".to_string(), "pt".to_string()],
        user_preferred_language: Some("en".to_string()),
        hooks: typefix::core::config::HooksConfig {
            keyboard_enabled: true,
            mode: typefix::core::config::HookMode::System,
            target_app: None,
        },
        ..typefix::core::config::Config::default()
    };

    typefix::init(&config).expect("init failed");

    let hook_config = typefix::hooks::platform::HookConfig {
        enabled: true,
        mode: typefix::hooks::platform::HookMode::System,
    };

    let hook = typefix::hooks::platform::create_hook(hook_config)
        .expect("Failed to create hook");

    hook.start().expect("Failed to start hook");

    tracing::info!("Hook started, building pipeline...");

    let pipeline = typefix::pipeline::TypeFixPipeline::new(
        typefix::pipeline::PipelineConfig::default()
    );
    {
        let state_arc = typefix::get_state();
        let state = state_arc.read();

        tracing::info!("Active language: {}", state.active_language);
        tracing::info!("Dictionaries: {:?}", state.dictionaries.keys().collect::<Vec<_>>());
        tracing::info!("Error maps: {:?}", state.error_maps.keys().collect::<Vec<_>>());

        for (lang, dict) in &state.dictionaries {
            pipeline.add_dictionary(lang, Arc::clone(dict));
        }
        for (lang, sw) in &state.stopwords {
            pipeline.add_stopwords(lang, Arc::clone(sw));
        }
        for (lang, em) in &state.error_maps {
            pipeline.add_error_map(lang, Arc::clone(em));
        }
        pipeline.set_language(&state.active_language);
    }

    tracing::info!("Pipeline ready. Type in any window. Press Ctrl+C to stop.");
    tracing::info!("Try typing 'teh ' (with space at end) in Notepad.");

    let receiver = hook.receiver();
    let mut event_count: u64 = 0;

    loop {
        let event = match receiver.recv() {
            Ok(event) => event,
            Err(_) => {
                tracing::info!("Hook receiver disconnected, shutting down...");
                break;
            }
        };

        event_count += 1;

        match &event.event {
            KeyEvent::Char(ch) => {
                tracing::info!("[EVENT #{event_count}] KeyEvent::Char('{ch}')");

                // Feed to pipeline
                let result = pipeline.push(*ch);

                match result {
                    Some(ref pr) => {
                        tracing::info!(
                            "[PIPELINE] Word extracted: '{}', corrected: {:?}",
                            pr.original,
                            pr.corrected
                        );
                        if let Some(ref corrected) = pr.corrected {
                            tracing::warn!(
                                "[CORRECTION] '{}' -> '{}'. Sending backspaces + corrected text.",
                                pr.original,
                                corrected
                            );
                            let backspaces = pr.original.chars().count();
                            if let Err(e) = hook.send_correction_atomic(backspaces, corrected, 0) {
                                tracing::error!("Failed to send correction atomically: {}", e);
                            }
                        }
                    }
                    None => {
                        tracing::debug!("[PIPELINE] Char '{ch}' buffered (no word yet). Buffer: '{}'", pipeline.buffer_contents());
                    }
                }
            }
            KeyEvent::Special(key) => {
                tracing::info!("[EVENT #{event_count}] KeyEvent::Special({key:?})");
                match key {
                    SpecialKey::Backspace => pipeline.clear(),
                    SpecialKey::Enter | SpecialKey::Tab | SpecialKey::Escape => {
                        let _ = pipeline.push(' ');
                    }
                    _ => {}
                }
            }
            KeyEvent::Control(key) => {
                tracing::debug!("[EVENT #{event_count}] KeyEvent::Control({key:?})");
            }
        }
    }
}
