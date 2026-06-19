# TypeFix v0.1.0 — Auditoría Multi-Perspectiva para Producción

**Fecha:** 2026-06-19
**Workspace:** `T:\Kiroku\minimax\proyectos\typefix`
**Versión auditada:** `typefix 0.1.0` (declarada "production-ready" en `docs/final-review-2026-06-16.md:107`)
**Casos de uso declarados:** EHR/HIPAA, legaltech, contact centers (`PRODUCT.md`)

---

## TL;DR — Veredicto consolidado de las 4 perspectivas

| Perspectiva | Veredicto | Razón principal |
|---|---|---|
| **QA** | 🔴 **NO listo** | `cargo clippy -D warnings` falla con 4 errores; tests reales = 208 (no 123 como dice el review); 0 tests E2E del flujo daemon; hooks no probados |
| **Ciberseguridad** | 🔴 **NO listo (riesgo legal HIPAA/GDPR)** | Hooks nativos son stubs; `log_keystrokes` puede capturar PHI sin filtro; claim "100% safe Rust" es **falso** (17 bloques `unsafe` en producción); CVSS 9.8 (R1) + 9.1 (R5 + R18) |
| **Arquitectura** | 🔴 **NO listo** | 3 hooks nativos no funcionan (Windows envía eventos a canal muerto, Linux/macOS panic en `receiver()`); SLO de <10MB violable al primer diccionario real (`HashMap<char,TrieNode>`); "0 allocs en hot path" es marketing, no ingeniería (~5 allocs/keystroke) |
| **Rust Senior Dev** | 🔴 **NO listo** | 5 `panic!` activos en producción que **crashean el daemon** en cuanto recibe el primer keystroke en Linux/macOS; `static mut HOOK_SENDER` sin sincronización = data race UB; `find_similar` O(n²) sobre Vec; 6 deps declaradas y no usadas |

**Consenso 4/4: NO PUBLICAR COMO PRODUCTION-READY.** El propio `docs/risk-register.md` reconoce 4 riesgos de severidad 9 (crítica) en estado **Activo**, pero el `docs/final-review-2026-06-16.md:115` firma "APPROVED ✅" — hay una **contradicción material** entre los dos documentos.

Recomendación: re-etiquetar como `0.1.0-rc1`, publicar el risk register como release notes, abrir 4 issues P0 bloqueantes, y bloquear cualquier release público hasta cerrar los 8 must-fix.

---

## 1. La contradicción central: `final-review` vs realidad del código

El comité que firmó el "APPROVED" del 2026-06-16 **no ejecutó los gates que su propio `justfile` y CI declaran como bloqueantes**. Los 4 auditores confirman:

| Documento / Gate | Lo que declara | Lo que el código y la realidad muestran |
|---|---|---|
| `docs/final-review-2026-06-16.md:115` | "El proyecto está listo para producción" | 4 riesgos críticos (R1, R5, R10, R18) activos |
| `docs/final-review-2026-06-16.md:13` | "123 tests passing" | `cargo test --all-features` retorna **208 tests passing** (se añadieron 85 después sin re-aprobar) |
| `justfile:21` `cargo clippy --all-targets --all-features -- -D warnings` | (implícito: pasa) | **Falla con 4 errores** en `src/hooks/windows.rs` (`unused imports`, `unused variables`, `WindowsHook` no implementa `Debug`) |
| `docs/risk-register.md:455` | "Crítica (×9): 4 riesgos — R1, R5, R10, R18" | R1: hooks stub. R5: log de passwords. R10: diccionarios juguete. R18: `receiver()` panic. **Todos sin cerrar.** |
| `docs/issues-acumulados.md:48-51` | "🔴 Critical (3): P4-01/02/03 (hooks stub)" | Sin resolver |
| `PRODUCT.md:89` y `SECURITY.md:41` | "100% safe Rust (no `unsafe` in production)" | **Falso**: 17 bloques `unsafe` activos en `src/hooks/{windows,macos}.rs` y `src/memory.rs` |
| `PRODUCT.md:83` | "Cero alocaciones en hot path" | Falso: ~5 `String`/`HashMap` allocs por keystroke (`pipeline.rs:128-167`) |
| `README.md:30` | "<10MB RAM footprint" | Violable: Trie con `HashMap<char, TrieNode>` añade ~14MB de overhead puro para 50K palabras (antes de contar nada más) |
| `config.json` | (declara YAML) | `Config::from_file` solo parsea JSON (`config.rs:218-228`) — el `config.yaml` distribuido no se puede cargar |

---

## 2. Los 8 hallazgos críticos consensuados (must-fix antes de cualquier release)

### 🔴 #1 — Hooks nativos no capturan teclas (R1)
- **Perspectivas que lo reportan:** QA, Seguridad, Arquitectura, Rust
- **Evidencia:**
  - `src/hooks/windows.rs:412-414`: `static mut HOOK_SENDER: Option<Sender<HookEvent>>` declarado pero **nunca inicializado** desde `start()`. El callback `keyboard_hook_proc` (línea 418) lee de un `Sender` global sin inicializar → **todos los keystrokes se pierden silenciosamente**.
  - `src/hooks/linux.rs:138-202`: `start()` conecta a XCB pero **no llama `xcb_grab_keyboard`** — solo hace `poll_for_event` y loggea. Es un no-op funcional.
  - `src/hooks/macos.rs:187-237`: el bloque completo de `CGEventTap::new(...)` está literalmente **comentado**. El thread solo duerme en `while !stop_flag { sleep(10ms) }`.
- **Impacto:** El binario compilado **NO captura teclas reales** en ninguna plataforma. El producto no funciona. CVSS 9.8.

### 🔴 #2 — `Hook::receiver()` causa crash determinista (R18)
- **Perspectivas:** QA, Seguridad, Arquitectura, Rust
- **Evidencia:** `panic!("receiver() called on ... - not implemented in this skeleton")` en:
  - `src/hooks/windows.rs:497`
  - `src/hooks/linux.rs:241`
  - `src/hooks/macos.rs:294`
  - Más dos stubs adicionales para builds cruzadas (`linux.rs:291`, `macos.rs:344`)
- **Impacto:** Cualquier llamada a `hook.receiver()` desde `src/main.rs:153` **mata el proceso al instante**. Combinado con `panic = "abort"` (`Cargo.toml:91`) → sin stack trace, sin recovery. CVSS 9.1.

### 🔴 #3 — `log_keystrokes` captura PHI/contraseñas sin filtro (R5)
- **Perspectivas:** Seguridad (CVSS 9.1), QA, Arquitectura
- **Evidencia:**
  - `config.json:34` `log_keystrokes: false` por default (bien), pero un override malicioso en config es trivial.
  - `src/hooks/windows.rs:454-456`: `tracing::debug!("KeyEvent: {:?}", hook_event)` loggea **el carácter en claro** sin redactar password fields.
  - **No existe filtro** de `EM_GETPASSWORDCHAR` (Windows), `isSecureTextField` (macOS), ni SecureInput/Wayland IME.
- **Impacto:** Violación directa de HIPAA Privacy Rule 45 CFR §164.502 (Minimum Necessary), GDPR Art. 9 (datos sanitarios), PCI-DSS 3.4. Si un usuario activa el flag para debug, las contraseñas, números de tarjeta y notas EHR se persisten a disco.

### 🔴 #4 — Diccionarios de juguete (R10)
- **Perspectivas:** QA, Seguridad, Arquitectura, Rust
- **Evidencia medida:**
  - `data/dictionaries/en.json` = **113 palabras**
  - `data/dictionaries/es.json` = **108 palabras**
  - `data/dictionaries/pt.json` = **66 palabras**
  - `data/errors/en.json` = 85 entradas
- **Problemas de integridad:**
  - `en.json:116` contiene `"城"` (carácter CJK aleatorio con freq=5000)
  - `en.json:103` y `en.json:117` duplican `"world"`
  - `data/errors/pt.json` no existe — `init()` lo carga con fallback silencioso
- **Impacto:** Caso de uso declarado = EHR (vocabulario ~50K términos técnicos ICD-10, SNOMED, nombres de medicamentos). El corrector **no reconocerá** `metformin`, `myocardial`, `E11.9`, etc. False sense of security. CVSS 9.4 (funcional).

### 🔴 #5 — Marketing vs código: claim "100% safe Rust" es falso (N1)
- **Perspectivas:** Seguridad, Rust
- **Evidencia:** 17 ocurrencias de `unsafe` en producción:
  - `src/hooks/windows.rs:6, 69, 72-79, 164, 308-311, 417-464`
  - `src/hooks/macos.rs:58, 187-237`
  - `src/memory.rs:70`
- **Impacto:** `PRODUCT.md:89` y `SECURITY.md:41` declaran: *"100% safe Rust (no `unsafe` blocks in production code)"*. Esto es **deceptive practice** y constituye misrepresentation con riesgo legal para clientes que firmaron BAA/DPA basándose en esa afirmación. CVSS 8.6.

### 🔴 #6 — `static mut` sin sincronización en callback de Windows (data race UB)
- **Perspectivas:** Seguridad, Rust
- **Evidencia:** `src/hooks/windows.rs:412-414`:
  ```rust
  static mut HOOK_SENDER: Option<Sender<HookEvent>> = None;
  static mut HOOK_LOG_KEYSTROKES: bool = false;
  ```
  El callback `keyboard_hook_proc` (línea 458) lee estas globales **sin lock, sin atomic, sin `OnceLock`**. Es `unsafe` implícito sobre memoria estática que el borrow checker no puede verificar. Con `codegen-units = 1` + LTO, el optimizador puede reordenar lecturas → comportamiento indefinido.
- **Impacto:** Data race observable, potencial UB. CRÍTICO para un binario que captura keystrokes de historiales clínicos.

### 🔴 #7 — `panic = "abort"` + hooks sin `catch_unwind` = crash determinista (R15)
- **Perspectivas:** Seguridad, Arquitectura, Rust
- **Evidencia:**
  - `Cargo.toml:91`: `panic = "abort"` en release.
  - Callbacks de hook en `windows.rs:418-464` no están envueltos en `std::panic::catch_unwind`.
  - Bug duplicado en `windows.rs:77-78`: la condición ALT se compara **dos veces con la misma tecla**, ALT derecho nunca se detecta.
- **Impacto:** Cualquier panic en `keyboard_hook_proc` (OOM en `kb_struct.time as u64`, `String::from_utf8_unchecked` con data envenenada) **mata el daemon sin stack trace**. Falla-silenciosa para un keylogger médico es peor que un crash ruidoso. CVSS 4.0 aislado, 8.0 una vez que R1 esté cerrado.

### 🔴 #8 — `cargo clippy -D warnings` falla — el gate declarado como bloqueante no pasa
- **Perspectivas:** QA, Rust
- **Evidencia (ejecutado 2026-06-19):**
  ```
  error: unused imports: `INPUT_TYPE` and `VIRTUAL_KEY`     src/hooks/windows.rs
  error: unused variable: `log_keystrokes`                   src/hooks/windows.rs:296
  error: unused variable: `sender`                           src/hooks/windows.rs:299
  error: type does not implement Debug                       src/hooks/windows.rs:43 (WindowsHook)
  ```
- **Impacto:** El comité que firmó "APPROVED" **no corrió el propio gate** que su `justfile:21` declara. El CI workflow pasa porque solo compila target Linux (donde `cfg(target_os = "windows")` no se compila), así que el error es invisible en CI actual. Es una **mentira técnica**.

---

## 3. Hallazgos altos consensuados (should-fix antes de 1.0)

| # | Hallazgo | Perspectivas | Evidencia clave | Acción |
|---|---|---|---|---|
| H1 | `find_similar` O(n²) sobre Vec (R6) | QA, Seguridad, Arquitectura, Rust | `src/core/trie.rs:204-230`: cada corrección copia todas las palabras y itera con Damerau | Cachear `all_words()` con `OnceCell` (1 línea); mediano plazo: BK-tree o Symspell |
| H2 | Auto-correct con `max_edit_distance=1` genera falsos positivos clínicos (R12) | Seguridad, Arquitectura, QA | `engine.rs:65`: "wont"→"want", "form"→"from" son clínica y legalmente distintos | Confidence threshold (freq > 10×), `suggestion_mode: true` por default, listas protegidas (ICD-10, medicamentos) |
| H3 | WH_KEYBOARD_LL requiere admin sin detección (R2) | Seguridad, QA | `windows.rs:309-347`: `SetWindowsHookExW` con `threadId=0` global | Detectar `GetLastError() == ERROR_ACCESS_DENIED`; ofrecer `mode: "application"` por default; firmar binario con EV cert |
| H4 | CGEventTap requiere Accessibility sin detección (R3) | Seguridad, QA | `macos.rs:179-237`: no llama `AXIsProcessTrusted()` | Validar antes de crear tap; watchdog que re-suscriba; notarizar binario |
| H5 | Sin soporte Wayland (R4) | Seguridad, QA, Arquitectura | `linux.rs:139-181`: solo XCB | Detectar `XDG_SESSION_TYPE=wayland`; documentar; investigar `wlr-input-inhibitor` |
| H6 | Deadlock latente en `notify_listeners` (R16) | Rust, Arquitectura | `buffer.rs:296-302`, `pipeline.rs:216-223`: callback ejecutado mientras se sostiene `read()` del `RwLock` | Clonar lista de listeners, soltar lock, iterar; o cambiar a `mpsc` channel |
| H7 | Trie `HashMap<char, TrieNode>` viola SLO <10MB a escala | Arquitectura, Rust | `core/trie.rs:14`: cada nodo aloca HashMap vacío (~48B × 300K nodos = 14MB) | Evaluar `hat-trie`, `marisa`, o `Vec<(char,Box<>)>` con sorted Vec |
| H8 | `user_errors` HashMap crece sin límite (R17) | Seguridad, QA | `static_map.rs:106-126`: sin LRU; `learn_from_user: true` por default (`config.json:27`) | LRU con cap de 1000; cambiar default a `false`; CLI `typefix stats` |
| H9 | Auto-correct ignora código/identificadores/ICD-10 (R19) | Seguridad, QA | `pipeline.rs:146-167`: sin heurística de field type | Si contiene `_`, `.`, dígitos, camelCase: no corregir; whitelist de patrones regex |
| H10 | `DamerauLevenshtein::distance_general` sin límite superior (R8) | Seguridad, Arquitectura, Rust | `damerau.rs:158-211`: matriz `(len+1)²` de `usize` = 8MB para 1000 chars; sin tope | Early-return si `len1 * len2 > 1_000_000`; cambiar a `u8` (8× ahorro) |
| H11 | `process_string` bloquea con paste >100K chars (R22) | QA, Arquitectura | `pipeline.rs:170-190`: itera char-by-char sin yield | Procesar en chunks; `tokio::spawn`; rechazar >100K chars con warning |
| H12 | `tests/integration.rs.bak` existe, no en `.gitignore` | Rust | 15219 bytes, residuo del rename `typo_engine` → `typefix`, no compila | Eliminar; añadir `*.rs.bak` a `.gitignore` |
| H13 | 6 dependencias declaradas y no usadas | Rust | `unicode-segmentation`, `unicode-normalization`, `tokio`, `config`, `quickcheck`, `proptest` (verificado con grep) | Implementar lo que R14/R22/R23 piden, o remover |
| H14 | Tests zombie: `assert!(results.is_empty() \|\| !results.is_empty())` | QA | `tests/integration_test.rs:269, 285, 373, 374, 435, 539` | Reemplazar con asserts reales o eliminar |
| H15 | `config.yaml` distribuido pero `Config::from_file` solo parsea JSON | QA, Rust | `config.rs:218-228` vs `config.yaml` versionado | Implementar parser YAML (añadir `serde_yaml`) o eliminar `config.yaml` |

---

## 4. Resumen por perspectiva

### 4.1 QA — Test Engineer

**Foco:** cobertura, CI/CD readiness, regresiones, automation.

- **Estado real de `cargo test --all-features`:** 208 tests passing (no 123 como declaraba el review). El delta de +85 tests se añadió entre el "approved" y hoy sin re-aprobar.
- **`cargo clippy -D warnings` falla** con 4 errores en `src/hooks/windows.rs`. El gate que el CI declara como bloqueante **no pasa localmente**.
- **Cobertura por módulo:**
  - ✅ Buena: `trie.rs`, `damerau.rs`, `engine.rs`, `config.rs`, `language/resolver.rs`, `buffer.rs`
  - ⚠️ Mínima: `pipeline.rs` (5 tests), `detector.rs` (6 tests)
  - ❌ **Cero tests** en `hooks/windows.rs`, `hooks/linux.rs`, `hooks/macos.rs` — los paths reales nunca se ejercitan
  - ❌ **Cero tests E2E** del flujo daemon: `init` → `start` → keystroke → correction → backspace injection
- **CI/CD gaps:**
  - Solo compila target Linux; Windows y macOS nunca se validan en CI
  - No hay SAST (`cargo-audit`, `cargo-deny`)
  - No hay benchmark regression gate
  - No hay fuzzing nightly
  - 28 `unwrap/expect` en producción que el script `scripts/ci-local.ps1` **no detecta correctamente** en builds donde se compila `windows-sys`
- **Veredicto:** No publicable. Rebautizar como `0.1.0-rc1`, abrir issues P0 por cada bloqueante.

### 4.2 Ciberseguridad — Security Engineer

**Foco:** superficie de ataque, cumplimiento HIPAA/GDPR, integridad, supply chain, threat model.

- **CVSS scores de los riesgos críticos:**
  - R1 (hooks stub): **9.8** (Crítica/Red)
  - R5 (log de PHI): **9.1** (Crítica/Local, PHI exfiltration)
  - R10 (diccionarios juguete): **9.4** (funcional, false sense of security)
  - R18 (receiver panic): **9.1** (Crítica, DoS persistente)
  - N1 (claim falso "100% safe Rust"): **8.6** (Alta, confianza/Compliance)
- **HIPAA — gap analysis:**
  - §164.312(a) Access Control: ❌ no auth al binario
  - §164.312(b) Audit Controls: ❌ no audit log real (tracing ≠ audit log inmutable)
  - §164.312(c) Integrity: ❌ auto-correct sin undo, sin versioning
  - §164.502(b) Minimum Necessary: ❌ captura universal de keystrokes (R5)
  - §164.514 De-identification: ❌ logs contienen PHI sin redacción
- **GDPR — gap analysis:**
  - Art. 5(1)(c) Data minimization: ❌
  - Art. 5(1)(e) Storage limitation: ❌ `user_errors.json` crece sin límite (R17)
  - Art. 17 Right to erasure: ❌ no hay UI/CLI para `clear_user_errors()`
  - Art. 25 Privacy by design: ❌ diseño opt-out, debería ser opt-in
- **Supply chain:** 16 dependencias runtime + 4 dev. **Ninguna con advisories activos.** `tokio` con `rt-multi-thread` está sobreespecificado para un binario que no usa async.
- **`SECURITY.md` gaps:** sin firma PGP / key fingerprint, sin SLA contractual, sin coordinated disclosure timeline.
- **Veredicto:** NO desplegar en producción bajo NINGUNA regulación (HIPAA, GDPR, PCI-DSS, SOX, FINRA). Las promesas de `PRODUCT.md:124-131` son marketing no implementado.

### 4.3 Arquitecto de Soluciones

**Foco:** objetivos declarados vs realidad, decisiones arquitectónicas, escalabilidad, deuda técnica.

- **SLO <1ms latencia — NO demostrado:** `benches/benchmarks.rs:5-37` mide solo `trie_insert`, `trie_search` (con 1 palabra), `buffer_push`, `damerau_one`. **No mide el hot path real** (keystroke → correction → injection). El `main.rs:281-346` `run_benchmarks()` calcula `chars_per_sec` con `test_text.len()` (bytes, no `chars().count()`), subestimando ~3-4× el throughput para texto no-ASCII.
- **SLO <10MB RAM — violable al primer diccionario real:** `HashMap<char, TrieNode>` por nodo añade ~14MB de overhead puro para 50K palabras. El test `test_dictionary_memory_tracker` (`memory.rs:323-333`) usa una fórmula inventada que no mide RSS real.
- **Claim "0 alocaciones en hot path" — FALSO:** ~5 allocs/keystroke (`pipeline.rs:128-167`: 4 `String::clone` + 1 `HashMap` para `find_similar`).
- **Cross-platform — ROTO en las 3 plataformas:**
  - Windows: callback envía a un canal global muerto (R18 variante)
  - Linux: `start()` no llama `xcb_grab_keyboard`, solo loggea
  - macOS: `CGEventTap` completamente comentado
- **Top 5 deuda arquitectónica:**
  1. Hook trait con `receiver() -> &Receiver` (firma fundamentalmente rota)
  2. Trie con `HashMap<char, TrieNode>` (peor layout de memoria posible)
  3. Estado global mutable `ENGINE_STATE` (anti-patrón para librería)
  4. `panic = "abort"` + `static mut` en callbacks
  5. Diccionarios triviales + JSON loading lento (100-200× más que mmap binario)
- **Roadmap a 1.0 realista:** 12-16 semanas, 2-3 ingenieros. Items mínimos: cerrar #1, #2, #4, #5 del top 5 de deuda.

### 4.4 Rust Senior Developer

**Foco:** idioms, ownership, errores, unsafe, performance, API pública, build.

- **`unwrap()`/`expect()` en producción:** Limpio. 1 `expect` defendible en `resolver.rs:39`. El resto (~35) están en `#[cfg(test)]` con `#[allow]` justificado.
- **`panic!()` en producción:** **5 activos** (`windows.rs:497`, `linux.rs:241,291`, `macos.rs:294,344`). Materialización directa de R18. Combinado con `panic = "abort"` = **crash determinista** del daemon en Linux/macOS.
- **`unsafe` blocks:** 17 ocurrencias, todas justificadas (FFI Win32/CoreGraphics + memory profiling). **Pero** sin `catch_unwind` en callbacks → R15 amplificado.
- **`static mut` sin sincronización:** `windows.rs:412-414` — **data race UB**. Es `unsafe` implícito que el borrow checker no puede verificar.
- **`find_similar` O(n²):** Confirmado en `trie.rs:204-230`. Con 100K palabras + `max_distance=2` ≈ 3.6M ops Damerau por corrección → p99 >100ms vs SLO <1ms.
- **`Vec::remove(0)` en hot path:** Confirmado en `detector.rs:118`. Trivially fixable con `VecDeque`.
- **Matriz Damerau sin límite:** Confirmado en `damerau.rs:158-211`. 8MB por llamada con strings de 1000 chars. Sin `try_reserve`, sin early-return.
- **API pública inflada:** `lib.rs:32-43` re-exports masivos con `pub use submodule::*`. Expone detalles internos (e.g., `CharBufferBuilder`, `MAX_BUFFER_SIZE`, `Delimiter`).
- **Bug de documentación:** `trie.rs:42-50` menciona un genérico `K` que **no existe** en la definición actual. Doc mintiendo.
- **Dependencias declaradas y no usadas:** 6 crates (`unicode-segmentation`, `unicode-normalization`, `tokio`, `config`, `quickcheck`, `proptest`). Verificado con grep. ~5-10MB de tiempo de compilación desperdiciado. R23 se queja de falta de normalización cuando la dep ya está.
- **`tests/integration.rs.bak`:** Existe, no en `.gitignore`. `.gitignore` actual lista `**/*.rs.bk` (sin la 'a'). Bypass del gitignore.
- **Veredicto:** Base del crate decente para 0.1. Capa de hooks no producción. Antes de 1.0: cerrar R18 (5 panics), R1 (hooks reales), decidir entre abort/catch_unwind, resolver `static mut`, cachear `all_words()`, limpiar `.gitignore`.

---

## 5. Recomendaciones priorizadas para llevar a producción

### 🔴 Must-fix antes de etiquetar 0.1.0 (8 items — ~6-10 semanas)

1. **Implementar hooks reales** (R1) y verificar con test E2E por plataforma (`tests/hook_e2e_{windows,linux,macos}.rs`). Cada test: arrancar hook → inyectar keystroke sintético → `receiver.recv_timeout(1s)` retorna `HookEvent` válida → `pipeline.push(event.ch)` produce corrección → `send_text("\x08" * N)` produce N backspaces.
2. **Cerrar R18:** re-arquitectura del hook trait. Cambiar `start()` para retornar `HookHandle { join: JoinHandle, receiver: Receiver<HookEvent> }`. Eliminar los 5 `panic!` activos. Eliminar `static mut HOOK_SENDER` en `windows.rs:412`.
3. **Cambiar `panic = "abort"` a `panic = "unwind"`** en `[profile.release]` y envolver callbacks en `std::panic::catch_unwind`. Crear perfil `[profile.daemon]` con `unwind` + `lto=true` para deployments server-side.
4. **Importar diccionarios reales** (≥10K palabras/idioma para en/es/pt) desde corpus público (OpenSubtitles, Wikipedia, Brown). Script reproducible en `scripts/build_dictionary.py`. Documentar tamaño mínimo en README.
5. **Forzar `log_keystrokes` a requerir opt-in CLI explícito** (`--log-keys`). Implementar filtro de password fields (Win32 `EM_GETPASSWORDCHAR`, macOS `isSecureTextField`, Linux/Wayland IME). Documentar HIPAA/GDPR implications en README y SECURITY.md.
6. **Corregir los 4 errores de clippy** + auditoría semanal con `cargo clippy --all-targets --all-features -- -D warnings` (el gate que el CI declara como bloqueante).
7. **Crear `data/errors/pt.json`** con ≥50 entradas. Hacer `init()` fail-fast si `supported_languages` referencia idioma sin datos.
8. **Corregir claims de marketing** en `PRODUCT.md:89` y `SECURITY.md:41`: cambiar "100% safe Rust" a "Safe Rust excepto en FFI de SO (windows, macOS, memory profiling) — auditado y minimizado". Actualizar "0 allocs en hot path" a "minimizadas, ~5 allocs por keystroke".

### 🟠 Should-fix antes de 1.0.0 (10 items — ~6-8 semanas)

9. Implementar `suggestion_mode: true` por default en producción (R12).
10. Heurística de no-corregir en código/identificadores (R19).
11. Resolver deadlock latente en `notify_listeners` (R16) — clonar lista de listeners antes de invocar.
12. LRU eviction en `StaticErrorMap::learn` con cap de 1000 (R17).
13. Cachear `Trie::all_words()` con `OnceCell` (R6 — 1 línea).
14. Límite duro en `DamerauLevenshtein::distance_general`: `if (len1+1)*(len2+1) > 1_000_000 { return max_dist + 1 }` (R8).
15. Cambiar `Vec<String>` a `VecDeque<String>` en `LanguageDetector::word_window` (R9).
16. Implementar `Vec::remove(0)` → `pop_front()` con `VecDeque` (R9).
17. Sanitizar `user_errors_path` y `data_path` en `config.rs:215-300`: rechazar paths absolutos fuera de whitelist.
18. Eliminar `tests/integration.rs.bak` y añadir `*.rs.bak` al `.gitignore`.
19. Eliminar o fortalecer los 6 tests zombie en `tests/integration_test.rs`.
20. Implementar property tests con `proptest` (que ya está en dev-deps) para `damerau_distance` (simetría, identidad, triángulo) y `find_similar`.

### 🟡 Roadmap post-1.0 (12 items)

- Soporte CJK (`unicode-segmentation` + `jieba-rs`) y RTL
- Soporte Wayland (`wlr-input-inhibitor`)
- Confidence threshold + audit log con undo
- Cross-compile automatizado (`Dockerfile` + `scripts/build-release.sh`)
- Métricas OTLP/Prometheus + health probe
- SBOM en cada release (`cargo-cyclonedx`)
- `cargo-audit` + `cargo-deny` en CI
- Notarización macOS + firma EV Windows
- Fuzzing nightly (`cargo-fuzz`)
- Refactor del estado global mutable `ENGINE_STATE`
- Reemplazar `HashMap<char, TrieNode>` por `hat-trie` o `marisa`
- Formato binario para diccionarios (`rkyv` + `memmap2`)

---

## 6. Decisión de release sugerida

| Acción | Justificación |
|---|---|
| **NO etiquetar `0.1.0` como `production-ready`** | Las 4 perspectivas coinciden; el propio `risk-register.md` reconoce 4 riesgos críticos activos |
| **Re-etiquetar como `0.1.0-rc1` (release candidate)** | Comunica transparencia: código público, no validado para casos de uso regulados |
| **Publicar `docs/risk-register.md` como release notes** | Disclosure responsable: "estos son los riesgos conocidos que el usuario debe evaluar" |
| **Abrir 8 issues P0 bloqueantes** con checklist de los must-fix | Tracking público, accountability |
| **Publicar como `preview técnico`** con disclaimer: "los hooks a nivel SO no están implementados en esta versión; use la API library o el modo REPL" | Reduce superficie legal; preserva el valor del núcleo (Trie, Buffer, Damerau, Resolver) que sí está sólido |
| **Asignar audit externo** (Trail of Bits, NCC Group, RadicallyOpenSecurity) antes de cualquier piloto clínico | Los hooks tocan datos PHI; un audit interno no es suficiente para BAA |
| **Establecer gate de release objetivo para 1.0.0:** `cargo clippy -D warnings` limpio + E2E hooks reales + diccionarios ≥10K + audit log HIPAA + property tests + fuzzing 1h clean | Definición operativa y medible de "production-ready" |

---

## 7. Conclusión para el release manager

TypeFix v0.1.0 tiene una **base algorítmica y arquitectónica razonable** para un producto open source en fase temprana — el Trie, el buffer circular, el corrector Damerau, el resolver de idioma con priorización user-pref/locale/default, y el pipeline son código de calidad publicable. La documentación interna (`risk-register.md`, `issues-acumulados.md`, `final-review-2026-06-16.md`) demuestra disciplina de equipo y honestidad intelectual.

**Pero la versión actual no cumple las promesas públicas de su `PRODUCT.md` y su `SECURITY.md`** — y la auditoría del 2026-06-16 que firma "APPROVED" lo hace sin ejecutar los gates declarados, sin abrir un terminal con los hooks reales, y sin notar que el clippy falla. Esa contradicción entre el documento de aprobación y la realidad del código es, en sí misma, el riesgo más serio: si el equipo publica `0.1.0` con la etiqueta "production-ready", el primer cliente que descubra que el binario no captura teclas (o que las contraseñas se loggean) generará un daño reputacional desproporcionado al valor del núcleo.

**Recomendación final:** publicar `0.1.0-rc1` con honesty disclosures, abrir los 8 P0 como issues públicos visibles, y comprometerse a un release `0.2.0` en 6-8 semanas que cierre los hooks reales + R18 + R10. Eso protege la reputación del equipo, mantiene la confianza de la comunidad open source, y crea el runway para llegar a `1.0.0` con un producto que cumpla lo que promete.

---

*Auditoría generada: 2026-06-19*
*4 perspectives: QA, Ciberseguridad, Arquitecto de Soluciones, Rust Senior Developer*
*Auditoría basada en lectura directa de `T:\Kiroku\minimax\proyectos\typefix`*
*Próxima revisión recomendada: tras resolver los 8 must-fix*
