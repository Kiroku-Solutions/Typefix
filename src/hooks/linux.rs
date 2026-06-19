//! Linux keyboard hook implementation
//!
//! Uses XCB for X11 keystroke capture. Requires X11 server running.

#[cfg(target_os = "linux")]
use super::{HookConfig, HookError, HookEvent, KeyboardHook, Modifiers, SpecialKey};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::Duration;

/// Linux-specific keyboard hook using XCB
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct LinuxHook {
    config: HookConfig,
    running: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    sender: Arc<Mutex<Option<Sender<HookEvent>>>>,
    receiver: Receiver<HookEvent>,
    hook_thread: Arc<parking_lot::Mutex<Option<thread::JoinHandle<()>>>>,
}

#[cfg(target_os = "linux")]
impl LinuxHook {
    /// Create a new Linux hook
    pub fn new(config: HookConfig) -> Result<Self, HookError> {
        let (tx, rx) = channel();
        Ok(Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            sender: Arc::new(Mutex::new(Some(tx))),
            receiver: rx,
            hook_thread: Arc::new(parking_lot::Mutex::new(None)),
        })
    }

    /// Convert XCB keycode to character
    #[expect(
        dead_code,
        reason = "keycode translator reserved for full XCB keyboard hook implementation; remove when wired up"
    )]
    fn keycode_to_char(keycode: u8, modifiers: &Modifiers) -> Option<char> {
        // Keycode to keysym mapping simplified
        // Full implementation would use xcb's key_symbols
        let base_keysym = match keycode {
            // Letters A-Z (keycodes 38-63 in standard X11)
            38..=63 => {
                let letter = (keycode - 38 + b'A') as char;
                if modifiers.shift ^ modifiers.caps_lock {
                    letter
                } else {
                    letter.to_ascii_lowercase()
                }
            }
            // Numbers 1-9, 0 (keycodes 10-19 in standard X11)
            10..=19 => {
                let num = (keycode - 10 + b'1') as char;
                if modifiers.shift {
                    match keycode {
                        10 => '!',
                        11 => '@',
                        12 => '#',
                        13 => '$',
                        14 => '%',
                        15 => '^',
                        16 => '&',
                        17 => '*',
                        18 => '(',
                        19 => ')',
                        _ => num,
                    }
                } else {
                    num
                }
            }
            // Space
            65 => ' ',
            // Return/Enter (keycode 36)
            36 => return Some('\n'),
            // Tab (keycode 23)
            23 => return Some('\t'),
            // Backspace (keycode 22)
            22 => return None, // Handled as special key
            _ => return None,
        };
        Some(base_keysym)
    }

    /// Map XCB keycode to special key
    #[expect(
        dead_code,
        reason = "keycode translator reserved for full XCB keyboard hook implementation; remove when wired up"
    )]
    fn keycode_to_special(keycode: u8) -> Option<SpecialKey> {
        match keycode {
            36 => Some(SpecialKey::Enter),
            23 => Some(SpecialKey::Tab),
            22 => Some(SpecialKey::Backspace),
            119 => Some(SpecialKey::Delete),
            9 => Some(SpecialKey::Escape),
            110 => Some(SpecialKey::Home),
            115 => Some(SpecialKey::End),
            112 => Some(SpecialKey::PageUp),
            117 => Some(SpecialKey::PageDown),
            111 => Some(SpecialKey::ArrowUp),
            116 => Some(SpecialKey::ArrowDown),
            113 => Some(SpecialKey::ArrowLeft),
            114 => Some(SpecialKey::ArrowRight),
            _ => None,
        }
    }
}

#[cfg(target_os = "linux")]
impl KeyboardHook for LinuxHook {
    fn start(&self) -> Result<(), HookError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(HookError::AlreadyRunning);
        }

        let running = Arc::clone(&self.running);
        let stop_flag = Arc::clone(&self.stop_flag);
        let log_keystrokes = self.config.log_keystrokes;
        let tx_clone = Arc::clone(&self.sender);

        // Spawn hook thread
        let handle = thread::spawn(move || {
            tracing::info!("Linux keyboard hook thread started");
            running.store(true, Ordering::SeqCst);

            // XCB event loop
            // Note: Full implementation would:
            // 1. Connect to X server
            // 2. Query keyboard
            // 3. Grab keyboard input
            // 4. Process events in loop
            // 5. Ungrab on stop

            // For this implementation, we check for DISPLAY and attempt connection
            let display_env = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());

            if let Ok(conn) = xcb::Connection::connect(Some(&display_env)) {
                let (conn, screen_num) = conn;
                tracing::info!(
                    "Connected to X server on {}, screen {}",
                    display_env,
                    screen_num
                );

                let setup = conn.get_setup();
                let screen = match setup.roots().nth(screen_num as usize) {
                    Some(s) => s,
                    None => {
                        tracing::error!("Screen not found");
                        return;
                    }
                };

                // Get keyboard input focus
                let _window = screen.root();

                // Grab keyboard (synchronous grab)
                // This would require XKB extension for proper Unicode handling
                // Simplified here - just demonstrate XCB connection

                while !stop_flag.load(Ordering::SeqCst) {
                    // Poll for events
                    if let Ok(Some(event)) = conn.poll_for_event() {
                        if log_keystrokes {
                            // Extract response type from UnknownEvent variant;
                            // other variants don't expose it directly.
                            let event_type = match &event {
                                xcb::Event::Unknown(unknown) => unknown.response_type() & 0x7f,
                                _ => 0,
                            };

                            // KeyPress = 2
                            if event_type == 2 {
                                if log_keystrokes {
                                    tracing::debug!("XCB KeyEvent: {:?}", event);
                                }
                                
                                // Simplified dummy mapping for MVP since full XKB translation
                                // is outside the scope of this file.
                                let timestamp = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0);
                                    
                                let hook_event = HookEvent {
                                    event: KeyEvent::Char('a'), // Dummy character
                                    timestamp,
                                    modifiers: Modifiers::default(),
                                };
                                
                                if let Ok(guard) = tx_clone.lock() {
                                    if let Some(tx) = &*guard {
                                        let _ = tx.send(hook_event);
                                    }
                                }
                            } else if event_type == 3 && log_keystrokes {
                                tracing::debug!("XCB KeyRelease: {:?}", event);
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            } else {
                tracing::warn!("Could not connect to X server. Is DISPLAY set?");
            }

            running.store(false, Ordering::SeqCst);
            tracing::info!("Linux keyboard hook thread stopped");
        });

        *self.hook_thread.lock() = Some(handle);
        tracing::info!("Linux keyboard hook started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), HookError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(HookError::NotRunning);
        }

        tracing::info!("Stopping Linux keyboard hook");
        self.stop_flag.store(true, Ordering::SeqCst);

        // Join hook thread
        if let Some(handle) = self.hook_thread.lock().take() {
            let _ = handle.join();
        }

        // Clear sender
        if let Ok(mut guard) = self.sender.lock() {
            *guard = None;
        }
        self.running.store(false, Ordering::SeqCst);

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn receiver(&self) -> &Receiver<HookEvent> {
        &self.receiver
    }
}

#[cfg(target_os = "linux")]
impl Drop for LinuxHook {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Stub implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
#[allow(missing_debug_implementations)]
pub struct LinuxHook {
    receiver: Receiver<HookEvent>,
}

#[cfg(not(target_os = "linux"))]
impl LinuxHook {
    pub fn new(_config: super::HookConfig) -> Result<Self, super::HookError> {
        Err(HookError::PlatformError(
            "Linux hook not available on this platform".to_string(),
        ))
    }
}

#[cfg(not(target_os = "linux"))]
use super::{HookError, KeyboardHook};

#[cfg(not(target_os = "linux"))]
impl KeyboardHook for LinuxHook {
    fn start(&self) -> Result<(), HookError> {
        Err(HookError::PlatformError(
            "Linux hook not available on this platform".to_string(),
        ))
    }

    fn stop(&mut self) -> Result<(), HookError> {
        Err(HookError::NotRunning)
    }

    fn is_running(&self) -> bool {
        false
    }

    fn receiver(&self) -> &Receiver<HookEvent> {
        &self.receiver
    }
}
