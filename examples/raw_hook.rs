use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, MSG,
    WH_KEYBOARD_LL, HHOOK, KBDLLHOOKSTRUCT, WM_KEYUP, WM_SYSKEYUP
};

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        
        let is_keyup = wparam.0 as u32 == WM_KEYUP || wparam.0 as u32 == WM_SYSKEYUP;
        let kind = if is_keyup { "UP" } else { "DOWN" };
        
        println!("HOOK EVENT: vkCode={}, type={}", kb.vkCode, kind);
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

fn main() {
    println!("Starting raw keyboard hook test...");
    unsafe {
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(hook_proc),
            None,
            0,
        );
        
        match hook {
            Ok(_) => println!("Hook installed successfully! Try typing now..."),
            Err(e) => {
                println!("Failed to install hook: {:?}", e);
                return;
            }
        }
        
        let mut msg: MSG = MSG::default();
        println!("Entering message loop...");
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
}
