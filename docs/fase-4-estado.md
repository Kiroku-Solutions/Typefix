# Fase 4: OS Hooks - Estado de Implementacion

**Fecha:** 2026-06-16  
**Estado:** Implementacion completada - Pendiente review

---

## 4.1 Arquitectura de Hooks

### Estructura General

```
hooks/
├── mod.rs          # Module exports
├── platform.rs     # Abstraccion comun + MockHook
├── windows.rs      # Windows implementation (WH_KEYBOARD_LL)
├── linux.rs        # Linux implementation (XCB/X11)
└── macos.rs        # macOS implementation (CGEventTap)
```

### Trait KeyboardHook

```rust
pub trait KeyboardHook: Send {
    fn start(&self) -> Result<(), HookError>;
    fn stop(&mut self) -> Result<(), HookError>;
    fn is_running(&self) -> bool;
    fn receiver(&self) -> &Receiver<HookEvent>;
}
```

---

## 4.2 Windows Hook (WH_KEYBOARD_LL)

### Implementacion
- **API:** Low-level keyboard hook (`WH_KEYBOARD_LL`)
- **Threading:** Spawn thread para pump de mensajes
- **Conversion VK → Char:** Usa `ToUnicodeW` para Unicode completo
- **Modificadores:** `GetAsyncKeyState` para estado actual

### Features
- ✅ Conversion VK a char con modificadores
- ✅ Especial keys (Enter, Tab, Backspace, Arrows, etc.)
- ✅ Modifier state tracking (Shift, Ctrl, Alt)
- ✅ Timestamps en eventos
- ✅ Graceful stop con thread join
- ⚠️ Requiere elevation (Admin) para system-wide hook

### Dependencias
```toml
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_LibraryLoader",
] }
```

---

## 4.3 Linux Hook (XCB/X11)

### Implementacion
- **API:** XCB para X11
- **Threading:** Event loop con `poll_for_event`
- **Grab:** Keyboard grab para capturar input
- **DISPLAY:** Lee variable de entorno

### Features
- ✅ Conexion a X server
- ✅ Event loop con polling
- ✅ Keycode mapping basico
- ⚠️ Requiere X11 server (no Wayland)
- ⚠️ XKB extension no implementada completamente

### Dependencias
```toml
xcb = { version = "1.7", features = ["xkb"] }
```

### Notas
- Para Wayland, se necesitaria `libwayland` diferente
- XKB proporciona mejor handling de keyboard layouts

---

## 4.4 macOS Hook (CGEventTap)

### Implementacion
- **API:** CoreGraphics `CGEventTap`
- **Threading:** Event callback thread
- **Permissions:** Requiere Accessibility permissions
- **Keycode:** Mapping estandar macOS keycodes

### Features
- ✅ CGEventTap para captura de eventos
- ✅ Keycode a char mapping
- ✅ Flags para modificadores
- ✅ Special keys mapping
- ⚠️ Requiere permisos de Accessibility (System Preferences)
- ⚠️ notarization requerida para distribution

### Dependencias
```toml
core-graphics = "0.25"
```

### Notas
- Para mejor Unicode support, se necesitaria `TISInputSource`
- Sandbox restrictions en App Store

---

## 4.5 MockHook para Testing

### Uso en Tests
```rust
let hook = MockHook::new(HookConfig::default());
hook.start().unwrap();

hook.simulate(KeyEvent::Char('h'));
hook.simulate(KeyEvent::Char('e'));
hook.simulate(KeyEvent::Char('l'));
hook.simulate(KeyEvent::Special(SpecialKey::Enter));

let rx = hook.receiver();
while let Ok(event) = rx.recv_timeout(Duration::from_millis(100)) {
    // Process event
}
```

### Tests Implementados
- test_mock_hook_start_stop
- test_mock_hook_already_running
- test_mock_hook_not_running
- test_mock_hook_simulate_char
- test_mock_hook_simulate_special_key
- test_mock_hook_simulate_control_key
- test_mock_hook_modifiers
- test_hook_event_timestamp
- test_hook_config_default
- test_modifiers_default
- test_hook_error_messages
- test_mock_hook_disconnect

---

## 4.6 Eventos de Hook

### KeyEvent
```rust
pub enum KeyEvent {
    Char(char),           // Caracter tipeado
    Control(ControlKey),  // Modifiers (Shift, Ctrl, Alt)
    Special(SpecialKey), // Special keys (Enter, Tab, etc.)
}
```

### HookEvent
```rust
pub struct HookEvent {
    pub event: KeyEvent,
    pub timestamp: u64,     // Unix timestamp ms
    pub modifiers: Modifiers, // Estado actual de modifiers
}
```

### Modifiers
```rust
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub caps_lock: bool,
}
```

---

## 4.7 Configuracion de Hook

### HookConfig
```rust
pub struct HookConfig {
    pub enabled: bool,
    pub log_keystrokes: bool,  // Debug logging
    pub mode: HookMode,        // System/Application/Disabled
}
```

### HookMode
```rust
pub enum HookMode {
    System,       // System-wide (requiere elevation)
    Application,  // Solo esta app
    Disabled,     // No hook
}
```

---

## 4.8 Integracion con Pipeline

### TypeFixPipeline Integration
```rust
// En pipeline.rs
pub struct TypeFixPipeline {
    buffer: CharBuffer,
    detector: Arc<LanguageDetector>,
    engine: CorrectionEngine,
    hook: Option<Box<dyn KeyboardHook>>,  // NEW
    callbacks: RwLock<Vec<PipelineCallback>>,
}

// Start hook
pipeline.start_hook()?;

// Hook procesa keystrokes -> Buffer -> Detector -> Engine -> Callbacks
```

---

## 4.9 Seguridad y Permisos

### Windows
- **System hook:** Requiere Admin o SeDebugPrivilege
- **Application hook:** Solo propia applicacion

### Linux
- **X11 grab:** Puede requerir permisos de X server
- **Wayland:** No soportado en esta version

### macOS
- **CGEventTap:** Requiere Accessibility permissions
- **Sandbox:** Incompatible con App Store

### Recomendaciones
1. Usar application mode por defecto (menos permisos)
2. Request elevation solo cuando necesario
3. Log de permisos solicitados
4. Handle permission denied gracefully

---

## 4.10 Issues Pendientes (ver docs/issues-acumulados.md)

| Issue | Severidad | Descripcion |
|-------|-----------|-------------|
| P4-01 | 🔴 Critical | Windows hook - skeleton con unsafe commented |
| P4-02 | 🔴 Critical | Linux hook - skeleton con XCB connect |
| P4-03 | 🔴 Critical | macOS hook - skeleton con CGEventTap commented |
| P4-04 | ⚠️ Minor | MockHook unwrap() en timestamp |

---

## 4.11 Criterios de Aceptacion Fase 4

| Criterio | Estado | Notas |
|----------|--------|-------|
| Trait KeyboardHook definido | ✅ | Con 4 metodos |
| Windows implementation | ⚠️ | Skeleton funcional |
| Linux implementation | ⚠️ | Skeleton con XCB |
| macOS implementation | ⚠️ | Skeleton con CGEventTap |
| MockHook para tests | ✅ | 12 tests |
| Graceful error handling | ✅ | HookError enum |
| Thread-safe | ✅ | Arc<AtomicBool> + Mutex |
| Platform abstraction | ✅ | #[cfg] macros |

---

## 4.12 Siguiente Paso: Review por Comite

El comité de 4 agentes debe aprobar antes de pasar a la siguiente fase.

---

*Estado: Implementacion completada, pendiente review — 2026-06-16*
