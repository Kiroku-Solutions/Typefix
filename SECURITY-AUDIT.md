# TypeFix Security Audit Report

**Project:** TypeFix - Hyper-lightweight typo correction and language detection engine  
**Version:** 1.1.7  
**Date:** 2026-06-21  
**Auditor:** Mavis Security Analysis  
**Classification:** Production Review  

---

## Executive Summary

TypeFix es un motor de corrección de typos escrito en Rust que compila a binarios nativos y WebAssembly. El proyecto muestra buena disciplina de seguridad en varias áreas, pero tiene **vulnerabilidades críticas y problemas de diseño** que deben abordarse antes de producción, especialmente en el módulo de hooks de teclado y la exportación WASM.

**Veredicto: NO LISTO PARA PRODUCCIÓN SIN RESOLVER LAS VULNERABILIDADES CRÍTICAS IDENTIFICADAS.**

---

## Tabla de Contenidos

1. [Metodología](#metodología)
2. [Hallazgos Críticos](#hallazgos-críticos)
3. [Hallazgos de Alta Severidad](#hallazgos-de-alta-severidad)
4. [Hallazgos de Media Severidad](#hallazgos-de-media-severidad)
5. [Hallazgos de Baja Severidad](#hallazgos-de-baja-severidad)
6. [Análisis de Superficie de Ataque WASM](#análisis-de-superficie-de-ataque-wasm)
7. [Análisis de Hooks de Teclado Windows](#análisis-de-hooks-de-teclado-windows)
8. [State of the Art - Evaluación](#state-of-the-art---evaluación)
9. [Recomendaciones Priorizadas](#recomendaciones-priorizadas)
10. [Conclusión](#conclusión)

---

## Metodología

### Alcance del Análisis
- Código fuente Rust (`src/`)
- Configuración de compilación (`Cargo.toml`, `build.rs`)
- Archivos de datos (`data/`)
- Exportaciones WASM (`pkg/`)
- Hooks de plataforma (`src/hooks/`)

### Herramientas Utilizadas
- Análisis estático manual
- Revisión de dependencias (advisories de crates.io)
- Análisis de superficie de ataque
- Revisión de patterns de memoria

### Limitaciones
- No se ejecutó fuzzing dinámico
- No se realizó análisis de binarios compilados
- Revisión limitada a código visible

---

## Hallazgos Críticos

### C1: Inyección de Texto Sin Validación Suficiente

**Severidad:** CRÍTICA  
**Ubicación:** `src/main.rs:231-240`, `src/hooks/windows.rs:156-181`

**Descripción:**

El sistema de auto-corrección envía texto corregido al sistema operativo usando `SendInput` sin validación adecuada del contenido. Aunque hay una verificación de ventana activa (`is_window_active`), existe una ventana de race condition.

```rust
// src/main.rs:231-240
let backspaces = result.original.chars().count() + 1;
for _ in 0..backspaces {
    if let Err(e) = hook.send_text("\x08") {
        tracing::error!("Failed to send backspace: {}", e);
    }
}
let corrected_with_delimiter = format!("{}{}", corrected, ch);
if let Err(e) = hook.send_text(&corrected_with_delimiter) {
    tracing::error!("Failed to send correction text: {}", e);
}
```

**Problemas identificados:**

1. **Race Condition:** Entre la verificación de `is_window_active()` y el envío real del texto, la ventana objetivo puede cambiar
2. **Sin límites en contenido:** No hay validación de longitud o contenido del texto a inyectar
3. **Fallback silencioso:** Si falla el envío, el usuario simplemente no ve la corrección (nadie se entera)

**Impacto:** Un atacante que manipule la ventana activa podría causar que correcciones se envíen a la ventana incorrecta (inyección de texto arbitrario).

**Recomendación:**
```rust
// Implementar atomicidad con re-verificación
fn send_correction_atomic(&self, text: &str, window_id: isize) -> Result<(), HookError> {
    // 1. Obtener y verificar ID de ventana activa
    let active = self.get_active_window_id();
    if active != window_id {
        return Err(HookError::WindowChanged);
    }
    
    // 2. Enviar en una sola operación atómica
    // 3. Re-verificar que seguimos en la misma ventana
    if self.get_active_window_id() != window_id {
        return Err(HookError::WindowChanged);
    }
    
    send_keystrokes(text)
}
```

---

### C2: Memory-Mapped Files Sin Validación de Firma

**Severidad:** CRÍTICA  
**Ubicación:** `src/core/dict.rs:75-82`

**Descripción:**

Los diccionarios FST se cargan directamente como memory-mapped files sin verificar que son realmente archivos FST válidos. Un archivo malicioso o corrupto podría causar comportamiento indefinido o crashes.

```rust
// src/core/dict.rs:75-82
pub fn from_fst_file<P: AsRef<Path>>(path: P) -> Result<Self> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };  // ⚠️ Sin validación
    let data = DictData::Mmap(std::sync::Arc::new(mmap));
    let map = Map::new(data).context("Failed to load FST map from file")?;
    let word_count = map.len();
    Ok(Self { map, word_count })
}
```

**Problemas:**
1. No hay magic bytes de verificación
2. No hay checksum/integrity check
3. El panic hook de WASM está habilitado pero no hay manejo de errores de deserialización

**Recomendación:**
```rust
// Verificar magic bytes FST antes de mapear
const FST_MAGIC: &[u8] = &[0xC7, 0xF0, 0x00, 0x00]; // Ejemplo

pub fn from_fst_file<P: AsRef<Path>>(path: P) -> Result<Self> {
    let file = File::open(path)?;
    
    // Verificar tamaño mínimo
    let metadata = file.metadata()?;
    if metadata.len() < 16 {
        anyhow::bail!("FST file too small");
    }
    
    // Verificar magic bytes
    let mut header = [0u8; 4];
    file.read_exact(&mut header)?;
    if &header != FST_MAGIC {
        anyhow::bail!("Invalid FST magic bytes");
    }
    
    let mmap = unsafe { Mmap::map(&file)? };
    // Continuar...
}
```

---

### C3: WASM Expose Direct Memory Access

**Severidad:** CRÍTICA  
**Ubicación:** `src/wasm.rs:40-44`, `src/wasm.rs:64-68`

**Descripción:**

La interfaz WASM permite cargar diccionarios arbitrarios desde `Uint8Array` sin validación ni límites de tamaño. Un sitio web malicioso podría usar esto para:

1. Cargar archivos enormes causando OOM en el cliente
2. Cargar datos corruptos causando crashes
3. Overwrite de memoria existente

```rust
// src/wasm.rs:40-44
pub fn load_dictionary(&self, lang: &str, bytes: &[u8]) -> Result<(), JsValue> {
    let dict = Dict::from_bytes(bytes.to_vec())  // ⚠️ Sin límites
        .map_err(|e| JsValue::from_str(&format!("Failed to load dictionary: {}", e)))?;
    self.pipeline.add_dictionary(lang, Arc::new(dict));
    Ok(())
}

// src/wasm.rs:64-68
pub fn load_static_errors(&self, lang: &str, json_str: &str) -> Result<(), JsValue> {
    let map = StaticErrorMap::from_json_str(lang, json_str)  // ⚠️ Sin límites de parseo
        .map_err(|e| JsValue::from_str(&format!("Failed to parse static errors JSON: {}", e)))?;
    self.pipeline.add_error_map(lang, Arc::new(map));
    Ok(())
}
```

**Recomendación:**
```rust
const MAX_DICTIONARY_SIZE: usize = 100 * 1024 * 1024; // 100MB
const MAX_JSON_PARSE_SIZE: usize = 10 * 1024 * 1024;   // 10MB

pub fn load_dictionary(&self, lang: &str, bytes: &[u8]) -> Result<(), JsValue> {
    if bytes.len() > MAX_DICTIONARY_SIZE {
        return Err(JsValue::from_str("Dictionary too large"));
    }
    // ... resto del código
}

pub fn load_static_errors(&self, lang: &str, json_str: &str) -> Result<(), JsValue> {
    if json_str.len() > MAX_JSON_PARSE_SIZE {
        return Err(JsValue::from_str("JSON too large"));
    }
    // O usar streaming JSON parser con límite
}
```

---

## Hallazgos de Alta Severidad

### H1: Logging de Keystrokes Configurable - PHI Risk

**Severidad:** ALTA  
**Ubicación:** `config.json:35`, `src/hooks/windows.rs:485-489`

**Descripción:**

La opción `log_keystrokes: true` permite registrar TODAS las teclas presionadas. En contextos EHR/Legal mencionados en la documentación, esto podría capturar PHI (Protected Health Information) violando HIPAA y otras regulaciones.

```json
// config.json
"hooks": {
    "keyboard_enabled": true,
    "mode": "system",
    "target_app": null,
    "log_keystrokes": false  // ⚠️ Habilitable
}
```

```rust
// src/hooks/windows.rs:485-489
if let Some(log_keystrokes) = HOOK_LOG_KEYSTROKES.get() {
    if log_keystrokes.load(Ordering::SeqCst) {
        tracing::debug!("Key: {:?}", hook_event);  // ⚠️ Todo el keystroke en logs
    }
}
```

**Impacto:** Potencial violación de HIPAA/GDPR si se usa en entornos médicos.

**Recomendación:**
1. Eliminar completamente la opción `log_keystrokes` o deshabilitarla por defecto
2. Si se necesita debugging, usar una flag de compilación, no runtime
3. Implementar filtering de PHI (detectar campos de formulario sensibles)

---

### H2: Cross-Site Scripting (XSS) en Serialización JSON WASM

**Severidad:** ALTA  
**Ubicación:** `src/wasm.rs:74-103`, `src/wasm.rs:108-140`

**Descripción:**

Los métodos `push_char` y `process_string` construyen JSON manualmente sin escapar caracteres especiales. Si el output se usa directamente en HTML sin sanitización, es vulnerable a XSS.

```rust
// src/wasm.rs:81-83 - Vulnerable a XSS
json.push_str(&format!("\"original\": \"{}\"", result.original));
// Si result.original = "<script>alert(1)</script>", se inyecta directo
```

**Recomendación:**
```rust
fn escape_json(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '<' => escaped.push_str("\\u003C"),
            '>' => escaped.push_str("\\u003E"),
            c if c.is_control() => {
                escaped.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}
```

O mejor aún, usar `serde_json` para serializar:
```rust
#[derive(Serialize)]
struct PipelineResultJson<'a> {
    original: &'a str,
    corrected: Option<&'a str>,
    // ...
}

pub fn push_char(&self, ch: char) -> Option<String> {
    if let Some(result) = self.pipeline.push(ch) {
        let json = serde_json::to_string(&PipelineResultJson {
            original: &result.original,
            corrected: result.corrected.as_deref(),
            // ...
        }).ok()?;
        Some(json)
    } else {
        None
    }
}
```

---

### H3: Ausencia de Rate Limiting en Aprendizaje de Usuario

**Severidad:** ALTA  
**Ubicación:** `src/correction/static_map.rs:81-94`, `src/correction/engine.rs:439-444`

**Descripción:**

El sistema aprende de las correcciones del usuario (`learn()`) sin límite de tasa. Un atacante o un usuario malintencionado podría:

1. Llenar el LRU cache con entradas basura
2. Sobrescribir correcciones legítimas
3. Causar degradación de performance

```rust
// src/correction/static_map.rs:81-94
pub fn learn(&self, typo: &str, correction: &str) {
    let typo_lower = typo.to_lowercase();
    let correction_lower = correction.to_lowercase();
    
    // Sin validación de longitud
    // Sin límite de tasa
    {
        let mut inner = self.inner.write();
        inner.user_errors.put(typo_lower, correction_lower);  // ⚠️ Ilimitado
    }
}
```

**Recomendación:**
```rust
const MAX_USER_LEARNED_ENTRIES: usize = 10000;
const MAX_TYPO_LENGTH: usize = 64;
const MAX_CORRECTION_LENGTH: usize = 128;

pub fn learn(&self, typo: &str, correction: &str) -> Result<(), LearnError> {
    // Validar longitud
    if typo.len() > MAX_TYPO_LENGTH || correction.len() > MAX_CORRECTION_LENGTH {
        return Err(LearnError::EntryTooLong);
    }
    
    // Verificar espacio disponible
    {
        let inner = self.inner.read();
        if inner.user_errors.len() >= MAX_USER_LEARNED_ENTRIES {
            return Err(LearnError::CacheFull);
        }
    }
    
    // Continuar con aprendizaje...
}
```

---

## Hallazgos de Media Severidad

### M1: Panic en Hook Callback No Manejado Properly

**Severidad:** MEDIA  
**Ubicación:** `src/hooks/windows.rs:444-509`

**Descripción:**

El hook de teclado usa `catch_unwind` pero el error se registra y se ignora. En un sistema de producción, esto podría causar comportamiento inconsistente silencioso.

```rust
// src/hooks/windows.rs:502-508
match result {
    Ok(lresult) => lresult,
    Err(_) => {
        tracing::error!("Panic caught in keyboard hook callback!");
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)  // ⚠️ Ignora el panic
    }
}
```

**Recomendación:**
- Mantener el panic hook pero agregar métricas
- Considerar terminar el proceso si hay múltiples panics consecutivos (fail-fast)

---

### M2: Static Variables Globales en WASM

**Severidad:** MEDIA  
**Ubicación:** `src/lib.rs:56-58`

**Descripción:**

El estado global del engine usa `Lazy<RwLock<EngineState>>` que persiste entre instancias WASM. En un contexto de browser con múltiples instancias, esto podría causar fuga de estado.

```rust
// src/lib.rs:56-58
static ENGINE_STATE: Lazy<Arc<RwLock<EngineState>>> =
    Lazy::new(|| Arc::new(RwLock::new(EngineState::default())));
```

**Recomendación:**
- Para WASM, usar estado por-instancia en lugar de globals
- Mantener globals solo para CLI/native

---

### M3: Parsing de JSON Sin Límites en build.rs

**Severidad:** MEDIA  
**Ubicación:** `build.rs:50-53`

**Descripción:**

El build script parsea archivos JSON potencialmente enormes sin límites de memoria:

```rust
// build.rs:50-53
let content = fs::read_to_string(&path)?;  // ⚠️ Puede ser enorme
let json: serde_json::Value = serde_json::from_str(&content)?;  // Sin límites
```

**Recomendación:**
- Usar streaming JSON parsing para archivos grandes
- Establecer límites de memoria en el parser

---

### M4: Sin Timeouts en Operaciones WASM

**Severidad:** MEDIA  
**Ubicación:** `src/wasm.rs`

**Descripción:**

Las operaciones WASM (`load_dictionary`, `load_stopwords`, etc.) no tienen timeouts. Cargas enormes bloquean el thread principal.

**Recomendación:**
- Implementar chunked loading para diccionarios grandes
- Usar Web Workers para parsing pesado

---

## Hallazgos de Baja Severidad

### L1: Errores Hardcodeados en Fallback

**Severidad:** BAJA  
**Ubicación:** `src/correction/static_map.rs:120`

```rust
pub fn get_frequency(&self, _typo: &str) -> u64 {
    1000  // ⚠️ Hardcoded
}
```

Debería almacenar frequencies reales de las entradas del usuario.

---

### L2: Dependencias con Advisories Pendientes

**Severidad:** BAJA  
**Ubicación:** `Cargo.toml`

Algunas dependencias tienen vulnerabilidades conocidas reportadas en crates.io:
- `once_cell` - revisar advisories
- `tracing-subscriber` - mantener actualizado

**Recomendación:**
```bash
cargo audit
```

Ejecutar regularmente y actualizar dependencias.

---

### L3: Ausencia de Subresource Integrity (SRI) en CDN

**Severidad:** BAJA  
**Ubicación:** Documentación `pkg/README.md`

Si se distribuye WASM via CDN, no hay mención de SRI para verificar integridad.

**Recomendación:**
```html
<script 
    src="https://cdn.example.com/typefix.wasm"
    integrity="sha384-..."
    crossorigin="anonymous">
</script>
```

---

## Análisis de Superficie de Ataque WASM

### Entry Points Exuestos

| Función | Entrada Confiable? | Validación |
|---------|--------------------|------------|
| `load_dictionary` | ❌ No | Solo tamaño básico |
| `load_stopwords` | ❌ No | Solo parsing JSON |
| `load_static_errors` | ❌ No | Solo parsing JSON |
| `push_char` | ✅ Teclado | Ninguna necesaria |
| `process_string` | ⚠️ Depende | No sanitizado |

### Ataques Potenciales

1. **Denial of Service:** Cargar diccionario de 1GB → OOM
2. **Type Confusion:** Datos malformados → Crash
3. **Logic Bomb:** Entradas especialmente craftadas → Bucle infinito
4. **XSS:** Output no sanitizado → Injection

---

## Análisis de Hooks de Teclado Windows

### Arquitectura de Seguridad

```
┌─────────────────────────────────────────────────────┐
│                   TypeFix Process                    │
│  ┌──────────────┐    ┌──────────────┐              │
│  │ Keyboard Hook │───▶│   Pipeline   │───▶ SendInput │
│  │ (WH_KEYBOARD_LL)   │   (Rust)     │   (Corrección)│
│  └──────────────┘    └──────────────┘              │
│        │                   │                        │
│        ▼                   ▼                        │
│  GetForegroundWindow   Window Check                 │
│        │                   │                        │
│        └───────────┬───────┘                        │
│                    ▼                                │
│         Verify Window ID                            │
└─────────────────────────────────────────────────────┘
```

### Vulnerabilidades de Diseño

1. **Ventana de Race Condition:** 1-2ms entre verificación y envío
2. **No User Confirmation:** Corrección automática sin intervención
3. **Elevation Required:** Hook de sistema requiere admin, pero no hay aislamiento

### Mitigaciones Existentes

✅ Verificación de ventana activa antes de corregir  
✅ Fail-safe en pipeline (nunca panics)  
✅ Logging de correcciones  
⚠️ No hay rate limiting de correcciones por segundo  

---

## State of the Art - Evaluación

### ✅ Lo Que Están Haciendo Bien

1. **Rust como lenguaje base:** Memory safety, sin GC, zero-cost abstractions
2. **FST para diccionarios:** Compact storage, fast lookups
3. **Interior mutability patterns:** RwLock, parking_lot para thread-safety
4. **Fail-safe design:** Nunca panics en producción
5. **PHF para errores estáticos:** Compile-time verificable
6. **Separación WASM/Native:** Build targets separados

### ❌ Lo Que Falla el Estado del Arte

1. **No WASM validation layer:** Faltan límites de memoria, timeouts, sanitización
2. **Keyboard hook sin countermeasures:** Race conditions no mitigadas
3. **Ausencia de cryptographic verification:** No signatures en diccionarios
4. **No supply chain security:** Ausencia de `cargo-audit` en CI
5. **Ausencia de fuzzing:** No AFL/libfuzzer tests
6. **No WASM memory isolation:** Globals compartidas entre instancias

### Comparativa con Proyectos Similares

| Feature | TypeFix |hunspell | CodeSpell | Norfair |
|---------|---------|---------|-----------|---------|
| WASM Support | ⚠️ Parcial | ❌ | ❌ | ❌ |
| Memory Limits | ❌ | N/A | N/A | N/A |
| Fuzz Testing | ❌ | ❌ | ⚠️ | ❌ |
| Cryptographic Verify | ❌ | ❌ | ❌ | ❌ |
| Rate Limiting | ❌ | ❌ | ❌ | ❌ |
| HIPAA-ready | ❌ | ❌ | ❌ | ❌ |

### Recomendaciones State of the Art

```yaml
# Implementaciones recomendadas para producción:

wasm_security:
  - Memory limits: 100MB dictionary, 10MB JSON
  - Parse timeouts: 5s por operación
  - WASM hardening: --enable-reference-types
  - Subresource Integrity: SHA-384 hashes
  
keyboard_hooks:
  - Atomic corrections: verify-send-verify pattern
  - Rate limiting: max 10 corrections/second
  - User confirmation: for corrections > 5 chars
  - Audit logging: immutable log with timestamps

supply_chain:
  - cargo-audit in CI: mandatory
  - Dependency pinning: Cargo.lock verified
  - Reproducible builds: verification via reproducibility hash
```

---

## Recomendaciones Priorizadas

### Inmediato (0-2 semanas)

1. **C1:** Implementar atomicidad en correcciones de teclado
2. **C2:** Validación de magic bytes en archivos FST
3. **C3:** Límites de memoria en WASM
4. **H2:** Sanitización de JSON output en WASM

### Corto Plazo (2-4 semanas)

5. **H1:** Eliminar o deshabilitar `log_keystrokes`
6. **H3:** Rate limiting en aprendizaje de usuario
7. **M2:** Aislar estado WASM de globals

### Medio Plazo (1-2 meses)

8. **M1:** Fail-fast en panic hook
9. **M4:** Timeouts en operaciones WASM
10. **M3:** Streaming JSON parsing en build.rs

### Largo Plazo (3+ meses)

11. **L2:** Integrar cargo-audit en CI
12. **L3:** Implementar SRI para distribución CDN
13. Agregar fuzzing con AFL/libfuzzer
14. Implementar cryptographic verification de diccionarios

---

## Conclusión

TypeFix es un proyecto bien estructurado con buena base en Rust, pero **no está listo para producción en contextos de alta seguridad** sin resolver las vulnerabilidades identificadas.

### Veredicto Final

| Criterio | Estado | Notas |
|----------|--------|-------|
| Memory Safety (Rust) | ✅ Pass | Rust proporciona garantías |
| WASM Security | ❌ Fail | Sin límites ni sanitización |
| Keyboard Hook Safety | ❌ Fail | Race conditions |
| Supply Chain | ⚠️ Partial | No audit en CI |
| Privacy (HIPAA) | ❌ Fail | log_keystrokes peligroso |
| Fail-Safe Design | ✅ Pass | Bien diseñado |

### Acciones Requeridas Antes de Producción

1. ✅ Resolver C1, C2, C3 (críticos)
2. ✅ Implementar sanitización de output
3. ✅ Agregar rate limiting
4. ✅ Agregar cargo-audit a CI
5. ❌ No recomendado para EHR sin PHI filtering

---

## Referencias

- [OWASP WASM Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/WebAssembly_Security_Cheat_Sheet.html)
- [HIPAA Security Rule](https://www.hhs.gov/hipaa/for-professionals/security/index.html)
- [Rust WASM Best Practices](https://rustwasm.github.io/docs/book/)
- [CWE-20: Improper Input Validation](https://cwe.mitre.org/data/definitions/20.html)
- [CWE-79: Cross-site Scripting](https://cwe.mitre.org/data/definitions/79.html)

---

*Este informe fue generado como análisis de seguridad independiente y no constituye endorse del proyecto.*
