//! Windows keyboard hook implementation
//!
//! Uses low-level keyboard hook (WH_KEYBOARD_LL) for system-wide keystroke capture.
//! This requires running with appropriate privileges.

#[cfg(target_os = "windows")]
use super::{HookConfig, HookError, HookEvent, KeyboardHook, Modifiers, SpecialKey};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "windows")]
use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use std::thread;
#[cfg(target_os = "windows")]
use std::time::Duration;

type HookThread = Arc<Mutex<Option<thread::JoinHandle<()>>>>;

/// Windows-specific keyboard hook using WH_KEYBOARD_LL
#[cfg(target_os = "windows")]
#[allow(
    missing_debug_implementations,
    reason = "Raw Windows handles and Arc<Mutex<JoinHandle>> don't impl Debug cleanly"
)]
pub struct WindowsHook {
    config: HookConfig,
    running: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    sender: Arc<Mutex<Option<Sender<HookEvent>>>>,
    hook_thread: HookThread,
}

#[cfg(target_os = "windows")]
impl WindowsHook {
    /// Create a new Windows hook
    pub fn new(config: HookConfig) -> Result<Self, HookError> {
        Ok(Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            sender: Arc::new(Mutex::new(None)),
            hook_thread: Arc::new(Mutex::new(None)),
        })
    }

    /// Get current modifier state
    #[allow(
        dead_code,
        reason = "reserved for full Windows hook implementation using GetAsyncKeyState"
    )]
    fn get_modifiers() -> Modifiers {
        // Simplified - would use GetAsyncKeyState in full implementation
        Modifiers {
            shift: false,
            ctrl: false,
            alt: false,
            caps_lock: false,
        }
    }

    /// Map VK to special key
    #[allow(
        dead_code,
        reason = "VK mapping table reserved for full Windows hook implementation"
    )]
    fn vk_to_special(vk: u32) -> Option<SpecialKey> {
        match vk {
            0x0D => Some(SpecialKey::Enter),
            0x09 => Some(SpecialKey::Tab),
            0x08 => Some(SpecialKey::Backspace),
            0x2E => Some(SpecialKey::Delete),
            0x1B => Some(SpecialKey::Escape),
            0x24 => Some(SpecialKey::Home),
            0x23 => Some(SpecialKey::End),
            0x21 => Some(SpecialKey::PageUp),
            0x22 => Some(SpecialKey::PageDown),
            0x26 => Some(SpecialKey::ArrowUp),
            0x28 => Some(SpecialKey::ArrowDown),
            0x25 => Some(SpecialKey::ArrowLeft),
            0x27 => Some(SpecialKey::ArrowRight),
            _ => None,
        }
    }

    /// Check if VK is a modifier key
    #[allow(
        dead_code,
        reason = "modifier key detection reserved for full Windows hook implementation"
    )]
    fn is_modifier_key(vk: u32) -> bool {
        matches!(
            vk,
            0x10 | 0x11 | 0x12 | // Shift/Ctrl/Alt
            0xA0 | 0xA1 | // Right/Left Shift
            0xA2 | 0xA3 | // Right/Left Ctrl
            0xA4 | 0xA5 // Right/Left Alt
        )
    }
}

#[cfg(target_os = "windows")]
impl KeyboardHook for WindowsHook {
    fn start(&self) -> Result<(), HookError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(HookError::AlreadyRunning);
        }

        let (tx, _rx) = channel::<HookEvent>();

        // Store sender
        *self
            .sender
            .lock()
            .map_err(|_| HookError::InitFailed("lock poisoned".into()))? = Some(tx);

        let running = Arc::clone(&self.running);
        let stop_flag = Arc::clone(&self.stop_flag);
        let _log_keystrokes = self.config.log_keystrokes;

        // Spawn hook thread
        let handle = thread::spawn(move || {
            tracing::info!("Windows keyboard hook thread started");
            running.store(true, Ordering::SeqCst);

            // Note: Full implementation would use SetWindowsHookExW to install
            // WH_KEYBOARD_LL hook and pump messages in a loop.
            //
            // The hook procedure would look like:
            //
            // unsafe extern "system" fn hook_proc(
            //     code: i32,
            //     wparam: WPARAM,
            //     lparam: LPARAM,
            // ) -> LRESULT {
            //     if code >= 0 {
            //         let kbstruct = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            //         // Process keyboard event...
            //     }
            //     CallNextHookEx(None, code, wparam, lparam)
            // }
            //
            // unsafe {
            //     let hook = SetWindowsHookExW(WH_KEYBOARD_LL, hook_proc, None, 0);
            // }

            // For now, simulate hook running
            while !stop_flag.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }

            running.store(false, Ordering::SeqCst);
            tracing::info!("Windows keyboard hook thread stopped");
        });

        if let Ok(mut guard) = self.hook_thread.lock() {
            *guard = Some(handle);
        }
        tracing::info!("Windows keyboard hook started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), HookError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(HookError::NotRunning);
        }

        tracing::info!("Stopping Windows keyboard hook");
        self.stop_flag.store(true, Ordering::SeqCst);

        // Join hook thread
        if let Ok(mut thread_guard) = self.hook_thread.lock() {
            if let Some(handle) = thread_guard.take() {
                let _ = handle.join();
            }
        }

        // Clear sender
        if let Ok(mut sender_guard) = self.sender.lock() {
            *sender_guard = None;
        }
        self.running.store(false, Ordering::SeqCst);

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn receiver(&self) -> &Receiver<HookEvent> {
        #[expect(
            clippy::panic,
            reason = "stub: full implementation will return a real receiver; remove when implemented"
        )]
        {
            panic!("receiver() called on WindowsHook - use MockHook for testing")
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsHook {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Stub implementation for non-Windows platforms
#[cfg(not(target_os = "windows"))]
#[allow(
    missing_debug_implementations,
    reason = "unit struct on non-Windows targets; no fields to debug"
)]
pub struct WindowsHook;

#[cfg(not(target_os = "windows"))]
impl WindowsHook {
    /// Construct a stub Windows hook on non-Windows platforms (always fails)
    pub fn new(_config: super::HookConfig) -> Result<Self, super::HookError> {
        Err(HookError::PlatformError(
            "Windows hook not available on this platform".into(),
        ))
    }
}

#[cfg(not(target_os = "windows"))]
use super::{HookError, KeyboardHook};

#[cfg(not(target_os = "windows"))]
impl KeyboardHook for WindowsHook {
    fn start(&self) -> Result<(), HookError> {
        Err(HookError::PlatformError(
            "Windows hook not available on this platform".into(),
        ))
    }

    fn stop(&mut self) -> Result<(), HookError> {
        Err(HookError::NotRunning)
    }

    fn is_running(&self) -> bool {
        false
    }

    fn receiver(&self) -> &Receiver<HookEvent> {
        #[expect(
            clippy::panic,
            reason = "stub implementation for non-Windows builds; never actually called"
        )]
        {
            panic!("receiver() called on stub WindowsHook")
        }
    }
}
