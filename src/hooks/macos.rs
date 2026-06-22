//! macOS keyboard hook implementation
//!
//! Uses CGEventTap for keystroke capture. Requires Accessibility permissions.

#[cfg(target_os = "macos")]
use super::{HookConfig, HookError, HookEvent, KeyboardHook, KeyEvent, Modifiers, SpecialKey};
#[cfg(target_os = "macos")]
use core_graphics::base::CGFloat;
#[cfg(target_os = "macos")]
use core_graphics::event::source::{CGEventSource, CGEventSourceStateID};
#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventTap, CGEventType, CGKeyCode};
#[cfg(target_os = "macos")]
use std::ptr;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
#[cfg(target_os = "macos")]
use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;

/// macOS-specific keyboard hook using CGEventTap
#[cfg(target_os = "macos")]
pub struct MacOSHook {
    config: HookConfig,
    running: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    sender: Arc<Mutex<Option<Sender<HookEvent>>>>,
    receiver: Receiver<HookEvent>,
    hook_thread: Option<thread::JoinHandle<()>>,
    event_tap: Option<CGEventTap>,
}

#[cfg(target_os = "macos")]
static EVENT_SENDER: OnceLock<Arc<Mutex<Option<Sender<HookEvent>>>>> = OnceLock::new();

#[cfg(target_os = "macos")]
impl MacOSHook {
    /// Create a new macOS hook
    pub fn new(config: HookConfig) -> Result<Self, HookError> {
        let (tx, rx) = channel();
        Ok(Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            sender: Arc::new(Mutex::new(Some(tx))),
            receiver: rx,
            hook_thread: None,
            event_tap: None,
        })
    }

    /// Convert CGKeyCode to character
    fn keycode_to_char(keycode: CGKeyCode, modifiers: &Modifiers) -> Option<char> {
        unsafe {
            let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;

            // Get the current keyboard layout
            let mut actual_string_length: usize = 0;
            let mut glyph = [0u16; 4];

            // Use CGEventKeyboardSetUnicodeString
            // Note: This is a simplified approach - full implementation would need
            // to properly handle keyboard layout and dead keys
            let result =
                core_graphics::sys::CGEventKeyboardSetUnicodeString(ptr::null(), 0, ptr::null());

            if result {
                // Fallback: use keycode-based mapping
                let ch = match keycode as u16 {
                    // Letters
                    0x00..=0x25 => {
                        let letter = (b'A' + keycode as u8) as char;
                        if modifiers.shift ^ modifiers.caps_lock {
                            letter
                        } else {
                            letter.to_ascii_lowercase()
                        }
                    }
                    // Numbers
                    0x1D..=0x26 => {
                        let num = (b'1' + (keycode as u8 - 0x1D)) as char;
                        if modifiers.shift {
                            match keycode as u8 {
                                0x1D => '!',
                                0x1E => '@',
                                0x1F => '#',
                                0x20 => '$',
                                0x21 => '%',
                                0x22 => '^',
                                0x23 => '&',
                                0x24 => '*',
                                0x25 => '(',
                                0x26 => ')',
                                _ => num,
                            }
                        } else {
                            num
                        }
                    }
                    // Space
                    0x31 => ' ',
                    // Return
                    0x24 => return Some('\n'),
                    // Tab
                    0x30 => return Some('\t'),
                    _ => return None,
                };
                Some(ch)
            } else {
                None
            }
        }
    }

    /// Map keycode to special key
    fn keycode_to_special(keycode: CGKeyCode) -> Option<SpecialKey> {
        match keycode as u16 {
            0x24 => Some(SpecialKey::Enter),
            0x30 => Some(SpecialKey::Tab),
            0x33 => Some(SpecialKey::Backspace),
            0x75 => Some(SpecialKey::Delete),
            0x35 => Some(SpecialKey::Escape),
            0x73 => Some(SpecialKey::Home),
            0x77 => Some(SpecialKey::End),
            0x74 => Some(SpecialKey::PageUp),
            0x79 => Some(SpecialKey::PageDown),
            0x7E => Some(SpecialKey::ArrowUp),
            0x7D => Some(SpecialKey::ArrowDown),
            0x7B => Some(SpecialKey::ArrowLeft),
            0x7C => Some(SpecialKey::ArrowRight),
            _ => None,
        }
    }

    /// Get modifiers from flags
    fn flags_to_modifiers(flags: CGFloat) -> Modifiers {
        let flags = flags as u64;
        Modifiers {
            shift: (flags & 0x01) != 0,
            ctrl: (flags & 0x02) != 0,
            alt: (flags & 0x04) != 0,
            caps_lock: (flags & 0x40) != 0,
        }
    }
}

#[cfg(target_os = "macos")]
impl KeyboardHook for MacOSHook {
    fn start(&self) -> Result<(), HookError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(HookError::AlreadyRunning);
        }

        let tx = self.sender.lock().unwrap().clone().ok_or(HookError::NotRunning)?;

        // Store sender globally for event callback
        EVENT_SENDER.get_or_init(|| Arc::new(Mutex::new(None)));
        if let Some(global_sender) = EVENT_SENDER.get() {
            if let Ok(mut guard) = global_sender.lock() {
                *guard = Some(tx);
            }
        }
        let running = Arc::clone(&self.running);
        let stop_flag = Arc::clone(&self.stop_flag);

        // Spawn hook thread
        let handle = thread::spawn(move || {
            tracing::info!("macOS keyboard hook thread started");
            running.store(true, Ordering::SeqCst);

            // Create event tap
            // CGEventTap requires Accessibility permissions
            // For simplicity, this demonstrates the structure without actual tap

            let event_mask =
                (1 << CGEventType::KeyDown as u64) |
                (1 << CGEventType::KeyUp as u64) |
                (1 << CGEventType::FlagsChanged as u64);

            let tap = CGEventTap::new(
                core_graphics::event::CGEventTapLocation::HID,
                core_graphics::event::CGEventTapPlacement::HeadInsertEventTap,
                core_graphics::event::CGEventTapOptions::DefaultTap,
                event_mask,
                |proxy, event_type, event| {
                    // Process keyboard event
                    let keycode = event.get_integer_value_field(
                        core_graphics::event::EventField::KEYBOARD_EVENT_KEYCODE
                    ) as CGKeyCode;

                    let flags = event.get_flags() as CGFloat;
                    let mods = MacOSHook::flags_to_modifiers(flags);

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);

                    match event_type {
                        CGEventType::KeyDown => {
                            if let Some(ch) = MacOSHook::keycode_to_char(keycode, &mods) {
                                let hook_event = HookEvent {
                                    event: KeyEvent::Char(ch),
                                    timestamp,
                                    modifiers: mods,
                                    window_id: 0,
                                };
                                if let Some(global_sender) = EVENT_SENDER.get() {
                                    if let Ok(guard) = global_sender.lock() {
                                        if let Some(tx) = &*guard {
                                            let _ = tx.send(hook_event);
                                        }
                                    }
                                }
                            } else if let Some(special) = MacOSHook::keycode_to_special(keycode) {
                                let hook_event = HookEvent {
                                    event: KeyEvent::Special(special),
                                    timestamp,
                                    modifiers: mods,
                                    window_id: 0,
                                };
                                if let Some(global_sender) = EVENT_SENDER.get() {
                                    if let Ok(guard) = global_sender.lock() {
                                        if let Some(tx) = &*guard {
                                            let _ = tx.send(hook_event);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }

                    Some(event.clone())
                },
            );

            while !stop_flag.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }

            running.store(false, Ordering::SeqCst);
            tracing::info!("macOS keyboard hook thread stopped");
        });

        self.hook_thread = Some(handle);
        tracing::info!("macOS keyboard hook started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), HookError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(HookError::NotRunning);
        }

        tracing::info!("Stopping macOS keyboard hook");
        self.stop_flag.store(true, Ordering::SeqCst);

        // Disable event tap
        if let Some(tap) = self.event_tap.take() {
            tap.invalidate();
        }

        // Join hook thread
        if let Some(handle) = self.hook_thread.take() {
            let _ = handle.join();
        }

        // Clear sender
        if let Ok(mut guard) = self.sender.lock() {
            *guard = None;
        }
        if let Some(sender) = EVENT_SENDER.get() {
            if let Ok(mut guard) = sender.lock() {
                *guard = None;
            }
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

#[cfg(target_os = "macos")]
impl Drop for MacOSHook {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Stub implementation for non-macOS platforms
#[cfg(not(target_os = "macos"))]
#[allow(missing_debug_implementations)]
pub struct MacOSHook {
    receiver: Receiver<HookEvent>,
}

#[cfg(not(target_os = "macos"))]
impl MacOSHook {
    pub fn new(_config: super::HookConfig) -> Result<Self, super::HookError> {
        Err(HookError::PlatformError(
            "macOS hook not available on this platform".to_string(),
        ))
    }
}

#[cfg(not(target_os = "macos"))]
use super::{HookError, KeyboardHook};

#[cfg(not(target_os = "macos"))]
impl KeyboardHook for MacOSHook {
    fn start(&self) -> Result<(), HookError> {
        Err(HookError::PlatformError(
            "macOS hook not available on this platform".to_string(),
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
