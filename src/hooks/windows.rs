//! Windows keyboard hook implementation
//!
//! Uses low-level keyboard hook (WH_KEYBOARD_LL) for system-wide keystroke capture.
//! This requires running with appropriate privileges (Administrator).

#![allow(unsafe_code)]

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "windows")]
use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use std::thread;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_TYPE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput, VK_BACK, VK_CAPITAL, VK_CONTROL,
    VK_LSHIFT, VK_MENU as VK_ALT, VK_RCONTROL, VK_RETURN, VK_RSHIFT, VK_SHIFT,
    VIRTUAL_KEY,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PeekMessageW, TranslateMessage, MSG, WH_KEYBOARD_LL,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_UP,
};

#[cfg(target_os = "windows")]
use super::{HookConfig, HookError, HookEvent, KeyboardHook, KeyEvent, Modifiers, SpecialKey};

#[cfg(target_os = "windows")]
type HookThread = Arc<Mutex<Option<thread::JoinHandle<()>>>>;

/// Windows-specific keyboard hook using WH_KEYBOARD_LL
#[cfg(target_os = "windows")]
pub struct WindowsHook {
    config: HookConfig,
    running: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    sender: Arc<Mutex<Option<Sender<HookEvent>>>>,
    receiver: Option<Receiver<HookEvent>>,
    hook_thread: HookThread,
}

#[cfg(target_os = "windows")]
impl WindowsHook {
    /// Create a new Windows hook
    pub fn new(config: HookConfig) -> Result<Self, HookError> {
        let (tx, rx) = channel::<HookEvent>();

        Ok(Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            sender: Arc::new(Mutex::new(Some(tx))),
            receiver: Some(rx),
            hook_thread: Arc::new(Mutex::new(None)),
        })
    }

    /// Get current modifier state using GetAsyncKeyState
    #[allow(unsafe_code)]
    fn get_modifiers() -> Modifiers {
        Modifiers {
            shift: unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) } < 0
                || unsafe { GetAsyncKeyState(VK_LSHIFT.0 as i32) } < 0
                || unsafe { GetAsyncKeyState(VK_RSHIFT.0 as i32) } < 0,
            ctrl: unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } < 0
                || unsafe { GetAsyncKeyState(VK_RCONTROL.0 as i32) } < 0,
            alt: unsafe { GetAsyncKeyState(VK_ALT.0 as i32) } < 0
                || unsafe { GetAsyncKeyState(VK_ALT.0 as i32) } < 0,
            caps_lock: unsafe { GetAsyncKeyState(VK_CAPITAL.0 as i32) } != 0,
        }
    }

    /// Map VK to special key
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

    /// Convert VK code to character
    fn vk_to_char(vk: u32, shift: bool, caps: bool) -> Option<char> {
        // Handle letters A-Z (0x41-0x5A)
        if (0x41..=0x5A).contains(&vk) {
            let c = (vk as u8) as char;
            let uppercase = if caps { !shift } else { shift };
            return Some(if uppercase { c } else { c.to_ascii_lowercase() });
        }

        // Handle numbers 0-9 (0x30-0x39)
        if (0x30..=0x39).contains(&vk) {
            let chars = if shift {
                [')', '!', '@', '#', '$', '%', '^', '&', '*', '(']
            } else {
                ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']
            };
            return Some(chars[(vk - 0x30) as usize]);
        }

        // Handle numpad
        if (0x60..=0x69).contains(&vk) {
            return Some(char::from_u32((vk - 0x60) + b'0' as u32).unwrap_or('\0'));
        }

        // Handle punctuation
        let shifted = shift;
        match vk {
            0xBD => Some(if shifted { '_' } else { '-' }),
            0xBE => Some(if shifted { '+' } else { '=' }),
            0xDB => Some(if shifted { '{' } else { '[' }),
            0xDC => Some(if shifted { '|' } else { '\\' }),
            0xDD => Some(if shifted { '}' } else { ']' }),
            0xDE => Some(if shifted { '"' } else { '\'' }),
            0xBF => Some(if shifted { '?' } else { '/' }),
            0xC0 => Some(if shifted { '~' } else { '`' }),
            0xBA => Some(if shifted { ':' } else { ';' }),
            0xBC => Some(if shifted { '<' } else { ',' }),
            0xBB => Some(if shifted { '>' } else { '.' }),
            0x32 => Some(if shifted { '@' } else { '2' }),
            _ => None,
        }
    }

    /// Send text to the system using SendInput
    pub fn send_text(&self, text: &str) -> Result<(), HookError> {
        send_keystrokes(text)
    }
}

fn send_keystrokes(text: &str) -> Result<(), HookError> {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.chars().count() * 4);

    for c in text.chars() {
        match c {
            '\r' => send_enter(&mut inputs),
            '\n' => send_char(c, &mut inputs),
            '\x08' => send_backspace(&mut inputs),
            _ => send_char(c, &mut inputs),
        }
    }

    unsafe {
        let result = SendInput(
            &inputs,
            std::mem::size_of::<INPUT>() as i32,
        );
        if result as usize != inputs.len() {
            return Err(HookError::InjectionFailed(
                format!("SendInput sent {} of {} events", result, inputs.len())
            ));
        }
    }

    Ok(())
}

fn send_char(c: char, inputs: &mut Vec<INPUT>) {
    let needs_shift = c.is_uppercase() || "+_()!@#$%^&*{}|~\"<>?".contains(c);

    if needs_shift {
        send_modifier_key(VK_SHIFT, inputs, false);
    }

    if c == '\t' {
        send_vkey(VK_BACK, 0x09, inputs);
    } else if c == '\x1b' {
        send_vkey(VK_BACK, 0x1B, inputs);
    } else {
        send_unicode_char(c, inputs);
    }

    if needs_shift {
        send_modifier_key(VK_SHIFT, inputs, true);
    }
}

fn send_unicode_char(c: char, inputs: &mut Vec<INPUT>) {
    let down = KEYBDINPUT {
        dwFlags: KEYEVENTF_UNICODE,
        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
        wScan: c as u16,
        ..Default::default()
    };
    let up = KEYBDINPUT {
        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
        wScan: c as u16,
        ..Default::default()
    };

    inputs.push(INPUT {
        r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_TYPE(1),
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: down,
        },
    });
    inputs.push(INPUT {
        r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_TYPE(1),
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: up,
        },
    });
}

fn send_vkey(_vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, vkey: u32, inputs: &mut Vec<INPUT>) {
    let down = KEYBDINPUT {
        dwFlags: KEYBD_EVENT_FLAGS(0),
        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vkey as u16),
        wScan: 0,
        ..Default::default()
    };
    let up = KEYBDINPUT {
        dwFlags: KEYEVENTF_KEYUP,
        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vkey as u16),
        wScan: 0,
        ..Default::default()
    };

    inputs.push(INPUT {
        r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_TYPE(1),
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: down,
        },
    });
    inputs.push(INPUT {
        r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_TYPE(1),
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: up,
        },
    });
}

fn send_modifier_key(
    vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    inputs: &mut Vec<INPUT>,
    key_up: bool,
) {
    let mut ki = KEYBDINPUT {
        wVk: vk,
        wScan: 0,
        ..Default::default()
    };
    if key_up {
        ki.dwFlags = KEYEVENTF_KEYUP;
    } else {
        ki.dwFlags = KEYBD_EVENT_FLAGS(0);
    }

    inputs.push(INPUT {
        r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_TYPE(1),
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki,
        },
    });
}

fn send_backspace(inputs: &mut Vec<INPUT>) {
    send_vkey(VK_BACK, VK_BACK.0 as u32, inputs);
}

fn send_enter(inputs: &mut Vec<INPUT>) {
    send_vkey(VK_RETURN, 0x0D, inputs);
}

#[cfg(target_os = "windows")]
impl KeyboardHook for WindowsHook {
    fn start(&self) -> Result<(), HookError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(HookError::AlreadyRunning);
        }

        let running = Arc::clone(&self.running);
        let stop_flag = Arc::clone(&self.stop_flag);
        let log_keystrokes = self.config.log_keystrokes;

        // Get sender from self.sender
        let sender = self.sender.lock()
            .map_err(|_| HookError::InitFailed("lock poisoned".into()))?
            .clone()
            .ok_or_else(|| HookError::InitFailed("sender not initialized".into()))?;

        // Spawn hook thread
        let handle = thread::spawn(move || {
            tracing::info!("Windows keyboard hook thread started");

            unsafe {
                let hook_result = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc as unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT),
                    None,
                    0,
                );

                match hook_result {
                    Ok(hook_handle) => {
                        tracing::info!("Windows keyboard hook installed successfully");
                        running.store(true, Ordering::SeqCst);

                        // Message loop - use PeekMessage with timeout
                        let mut msg: MSG = MSG::default();
                        loop {
                            if stop_flag.load(Ordering::SeqCst) {
                                tracing::info!("Stop flag set, exiting message loop");
                                break;
                            }

                            // Poll for messages with short timeout
                            if PeekMessageW(&mut msg, None, 0, 0, windows::Win32::UI::WindowsAndMessaging::PM_REMOVE).as_bool() {
                                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                                    tracing::info!("WM_QUIT received");
                                    break;
                                }
                                let _ = TranslateMessage(&msg);
                            } else {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }
                        }

                        let _ = UnhookWindowsHookEx(hook_handle);
                        tracing::info!("Windows keyboard hook uninstalled");
                    }
                    Err(e) => {
                        tracing::error!("Failed to install hook: {}", e);
                    }
                }
            }

            running.store(false, Ordering::SeqCst);
            tracing::info!("Windows keyboard hook thread stopped");
        });

        if let Ok(mut guard) = self.hook_thread.lock() {
            *guard = Some(handle);
        }

        thread::sleep(std::time::Duration::from_millis(50));

        if self.running.load(Ordering::SeqCst) {
            tracing::info!("Windows keyboard hook started");
            Ok(())
        } else {
            Err(HookError::InitFailed("Hook thread failed to start".into()))
        }
    }

    fn stop(&mut self) -> Result<(), HookError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(HookError::NotRunning);
        }

        tracing::info!("Stopping Windows keyboard hook");
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Ok(mut thread_guard) = self.hook_thread.lock() {
            if let Some(handle) = thread_guard.take() {
                let _ = handle.join();
            }
        }

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
        self.receiver.as_ref().expect("receiver not initialized")
    }

    fn send_text(&self, text: &str) -> Result<(), HookError> {
        send_keystrokes(text)
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsHook {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Static storage for hook callback
#[cfg(target_os = "windows")]
static mut HOOK_SENDER: Option<Sender<HookEvent>> = None;
#[cfg(target_os = "windows")]
static mut HOOK_LOG_KEYSTROKES: bool = false;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb_struct = *(lparam.0 as *const KBDLLHOOKSTRUCT);

        let vk_code = kb_struct.vkCode;  // u32
        let flags = kb_struct.flags;     // KBDLLHOOKSTRUCT_FLAGS

        let is_keyup = (flags.0 & LLKHF_UP.0) != 0;

        let modifiers = WindowsHook::get_modifiers();

        let event = if is_keyup {
            // On keyup, emit backspace for delete behavior
            if vk_code == VK_BACK.0 as u32 {
                KeyEvent::Special(SpecialKey::Backspace)
            } else {
                // Skip keyup for regular characters
                return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
            }
        } else {
            // Keydown
            if let Some(special) = WindowsHook::vk_to_special(vk_code) {
                KeyEvent::Special(special)
            } else if let Some(ch) = WindowsHook::vk_to_char(vk_code, modifiers.shift, modifiers.caps_lock) {
                KeyEvent::Char(ch)
            } else {
                return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
            }
        };

        let hook_event = HookEvent {
            event,
            timestamp: kb_struct.time as u64,
            modifiers,
        };

        if HOOK_LOG_KEYSTROKES {
            tracing::debug!("KeyEvent: {:?}", hook_event);
        }

        if let Some(ref sender) = HOOK_SENDER {
            let _ = sender.send(hook_event);
        }
    }

    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

// Stub implementation for non-Windows platforms
#[cfg(not(target_os = "windows"))]
#[allow(missing_debug_implementations)]
pub struct WindowsHook;

#[cfg(not(target_os = "windows"))]
impl WindowsHook {
    pub fn new(_config: super::HookConfig) -> Result<Self, super::HookError> {
        Err(super::HookError::PlatformError(
            "Windows hook not available on this platform".into(),
        ))
    }
}

#[cfg(not(target_os = "windows"))]
impl super::KeyboardHook for WindowsHook {
    fn start(&self) -> Result<(), super::HookError> {
        Err(super::HookError::PlatformError(
            "Windows hook not available on this platform".into(),
        ))
    }

    fn stop(&mut self) -> Result<(), super::HookError> {
        Err(super::HookError::NotRunning)
    }

    fn is_running(&self) -> bool {
        false
    }

    fn receiver(&self) -> &Receiver<HookEvent> {
        panic!("receiver() called on stub WindowsHook")
    }
}
