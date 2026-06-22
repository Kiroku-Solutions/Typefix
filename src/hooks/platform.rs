//! Platform abstraction for keyboard hooks
//!
//! Provides a unified interface for keyboard hooks across all platforms.

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(test)]
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;

/// Keyboard event types
#[derive(Debug, Clone)]
pub enum KeyEvent {
    /// A character was typed
    Char(char),
    /// A control key was pressed
    Control(ControlKey),
    /// A special key was pressed
    Special(SpecialKey),
}

/// Control keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKey {
    /// Shift modifier key
    Shift,
    /// Control modifier key
    Ctrl,
    /// Alt modifier key
    Alt,
    /// Caps Lock toggle key
    CapsLock,
    /// Num Lock toggle key
    NumLock,
}

/// Special keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    /// Enter / Return key
    Enter,
    /// Tab key
    Tab,
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,
    /// Escape key
    Escape,
    /// Home key
    Home,
    /// End key
    End,
    /// Page Up key
    PageUp,
    /// Page Down key
    PageDown,
    /// Up arrow key
    ArrowUp,
    /// Down arrow key
    ArrowDown,
    /// Left arrow key
    ArrowLeft,
    /// Right arrow key
    ArrowRight,
}

/// Hook event
#[derive(Debug, Clone)]
pub struct HookEvent {
    /// Event type
    pub event: KeyEvent,
    /// Timestamp in milliseconds
    pub timestamp: u64,
    /// Modifier state
    pub modifiers: Modifiers,
    /// Window ID where the event occurred
    pub window_id: isize,
}

/// Keyboard modifiers state
#[derive(Debug, Clone, Default)]
pub struct Modifiers {
    /// Shift key is currently held
    pub shift: bool,
    /// Control key is currently held
    pub ctrl: bool,
    /// Alt key is currently held
    pub alt: bool,
    /// Caps Lock is currently active
    pub caps_lock: bool,
}

/// Hook configuration
#[derive(Debug, Clone)]
pub struct HookConfig {
    /// Enable hook
    pub enabled: bool,
    /// Hook mode
    pub mode: HookMode,
}

/// Hook mode selecting the installation scope of the keyboard hook
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookMode {
    /// System-wide hook (requires elevation on Windows)
    System,
    /// Application-scoped hook
    Application,
    /// Hook disabled
    Disabled,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: HookMode::System,
        }
    }
}

/// Keyboard hook trait
pub trait KeyboardHook: Send {
    /// Start the hook
    fn start(&self) -> Result<(), HookError>;

    /// Stop the hook
    fn stop(&mut self) -> Result<(), HookError>;

    /// Check if hook is running
    fn is_running(&self) -> bool;

    /// Get event receiver
    fn receiver(&self) -> &Receiver<HookEvent>;

    /// Send text keystrokes to the system
    fn send_text(&self, text: &str) -> Result<(), HookError>;

    /// Check if window is still active
    fn is_window_active(&self, window_id: isize) -> bool {
        let _ = window_id;
        true
    }

    /// Atomically send backspaces and text, ensuring window hasn't changed.
    fn send_correction_atomic(&self, backspaces: usize, text: &str, window_id: isize) -> Result<(), HookError> {
        if !self.is_window_active(window_id) {
            return Err(HookError::InjectionFailed("Window changed".into()));
        }
        for _ in 0..backspaces {
            self.send_text("\x08")?;
        }
        if !self.is_window_active(window_id) {
            return Err(HookError::InjectionFailed("Window changed during backspace".into()));
        }
        self.send_text(text)
    }
}

/// Hook errors
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    /// Hook installation failed during startup
    #[error("Hook initialization failed: {0}")]
    InitFailed(String),
    /// Attempted to start a hook that is already running
    #[error("Hook already running")]
    AlreadyRunning,
    /// Attempted to use a hook that has not been started
    #[error("Hook not running")]
    NotRunning,
    /// The OS denied the privileges required to install the hook
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    /// A platform-specific error occurred while running the hook
    #[error("Platform error: {0}")]
    PlatformError(String),
    /// Keystroke injection failed
    #[error("Keystroke injection failed: {0}")]
    InjectionFailed(String),
}

/// Create platform-specific hook
#[cfg(target_os = "windows")]
pub fn create_hook(config: HookConfig) -> Result<Box<dyn KeyboardHook>, HookError> {
    Ok(Box::new(super::windows::WindowsHook::new(config)?))
}

/// Create a platform-specific Linux keyboard hook.
#[cfg(target_os = "linux")]
pub fn create_hook(config: HookConfig) -> Result<Box<dyn KeyboardHook>, HookError> {
    Ok(Box::new(super::linux::LinuxHook::new(config)?))
}

#[cfg(target_os = "macos")]
pub fn create_hook(config: HookConfig) -> Result<Box<dyn KeyboardHook>, HookError> {
    Ok(Box::new(super::macos::MacOSHook::new(config)?))
}

/// Mock hook for testing
#[allow(
    missing_debug_implementations,
    reason = "test mock contains Arc<AtomicBool> and channels; manual Debug adds no value"
)]
pub struct MockHook {
    #[allow(
        dead_code,
        reason = "config is only read in tests via MockHook::new(HookConfig::default())"
    )]
    config: HookConfig,
    running: Arc<AtomicBool>,
    sender: Option<Sender<HookEvent>>,
    receiver: Receiver<HookEvent>,
}

impl MockHook {
    /// Construct a new `MockHook` using the supplied configuration
    pub fn new(config: HookConfig) -> Self {
        let (sender, receiver) = channel();
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            sender: Some(sender),
            receiver,
        }
    }

    /// Simulate a keypress (for testing)
    pub fn simulate(&self, event: KeyEvent) {
        let hook_event = HookEvent {
            event,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            modifiers: Modifiers::default(),
            window_id: 0,
        };
        if let Some(ref sender) = self.sender {
            let _ = sender.send(hook_event);
        }
    }
}

impl KeyboardHook for MockHook {
    fn start(&self) -> Result<(), HookError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(HookError::AlreadyRunning);
        }
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), HookError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(HookError::NotRunning);
        }
        self.running.store(false, Ordering::SeqCst);
        // Drop sender to disconnect receiver
        self.sender = None;
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn receiver(&self) -> &Receiver<HookEvent> {
        &self.receiver
    }

    fn send_text(&self, _text: &str) -> Result<(), HookError> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]
#[allow(
    clippy::bool_assert_comparison,
    reason = "explicit false/true comparisons improve test readability"
)]
#[allow(unused_mut, reason = "mut bindings used in test assertions")]
#[allow(
    unused_variables,
    reason = "result variables used implicitly by side-effecting test code"
)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_hook_start_stop() {
        let mut hook = MockHook::new(HookConfig::default());
        assert!(!hook.is_running());

        hook.start().unwrap();
        assert!(hook.is_running());

        hook.stop().unwrap();
        assert!(!hook.is_running());
    }

    #[test]
    fn test_mock_hook_already_running() {
        let mut hook = MockHook::new(HookConfig::default());
        hook.start().unwrap();

        let result = hook.start();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HookError::AlreadyRunning));
    }

    #[test]
    fn test_mock_hook_not_running() {
        let mut hook = MockHook::new(HookConfig::default());

        let result = hook.stop();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HookError::NotRunning));
    }

    #[test]
    fn test_mock_hook_simulate_char() {
        let hook = MockHook::new(HookConfig::default());
        hook.start().unwrap();

        hook.simulate(KeyEvent::Char('h'));
        hook.simulate(KeyEvent::Char('e'));
        hook.simulate(KeyEvent::Char('l'));
        hook.simulate(KeyEvent::Char('l'));
        hook.simulate(KeyEvent::Char('o'));

        let rx = hook.receiver();
        let mut received = Vec::new();

        // Receive all events with timeout
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => received.push(event),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        assert_eq!(received.len(), 5);
        assert!(received
            .iter()
            .all(|e| matches!(e.event, KeyEvent::Char(_))));
    }

    #[test]
    fn test_mock_hook_simulate_special_key() {
        let hook = MockHook::new(HookConfig::default());
        hook.start().unwrap();

        hook.simulate(KeyEvent::Special(SpecialKey::Enter));

        let rx = hook.receiver();
        let event = rx.recv_timeout(Duration::from_millis(100)).unwrap();

        assert!(matches!(event.event, KeyEvent::Special(SpecialKey::Enter)));
    }

    #[test]
    fn test_mock_hook_simulate_control_key() {
        let hook = MockHook::new(HookConfig::default());
        hook.start().unwrap();

        hook.simulate(KeyEvent::Control(ControlKey::Shift));

        let rx = hook.receiver();
        let event = rx.recv_timeout(Duration::from_millis(100)).unwrap();

        assert!(matches!(event.event, KeyEvent::Control(ControlKey::Shift)));
    }

    #[test]
    fn test_mock_hook_modifiers() {
        let hook = MockHook::new(HookConfig::default());
        hook.start().unwrap();

        let modifiers = Modifiers {
            shift: true,
            ctrl: false,
            alt: false,
            caps_lock: true,
        };

        hook.simulate(KeyEvent::Char('A'));
        // In real implementation, modifiers would be set on the event

        let rx = hook.receiver();
        let event = rx.recv_timeout(Duration::from_millis(100)).unwrap();

        assert_eq!(event.modifiers.shift, false); // Default
        assert_eq!(event.modifiers.ctrl, false);
    }

    #[test]
    fn test_hook_event_timestamp() {
        let hook = MockHook::new(HookConfig::default());
        hook.start().unwrap();

        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        hook.simulate(KeyEvent::Char('a'));

        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let rx = hook.receiver();
        let event = rx.recv_timeout(Duration::from_millis(100)).unwrap();

        assert!(event.timestamp >= before);
        assert!(event.timestamp <= after);
    }

    #[test]
    fn test_hook_config_default() {
        let config = HookConfig::default();
        assert!(config.enabled);
        assert!(matches!(config.mode, HookMode::System));
    }

    #[test]
    fn test_modifiers_default() {
        let mods = Modifiers::default();
        assert!(!mods.shift);
        assert!(!mods.ctrl);
        assert!(!mods.alt);
        assert!(!mods.caps_lock);
    }

    #[test]
    fn test_hook_error_messages() {
        let err = HookError::InitFailed("test".to_string());
        assert!(err.to_string().contains("test"));

        let err = HookError::PermissionDenied("access".to_string());
        assert!(err.to_string().contains("access"));

        let err = HookError::PlatformError("x11".to_string());
        assert!(err.to_string().contains("x11"));
    }

    #[test]
    fn test_mock_hook_disconnect() {
        let mut hook = MockHook::new(HookConfig::default());
        hook.start().unwrap();
        hook.stop().unwrap();

        // Receiver should be disconnected after stop
        let rx = hook.receiver();
        let result = rx.recv_timeout(Duration::from_millis(10));
        assert!(matches!(result, Err(RecvTimeoutError::Disconnected)));
    }
}
