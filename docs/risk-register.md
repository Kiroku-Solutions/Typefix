# Risk Register

**Proyecto:** typefix
**Fecha:** 2026-06-16
**Versión analizada:** 0.1.0 (Phase 1-4 completadas)
**Mantenedor:** TypeFix Team

> Convención de severidad: **Crítica** (×9) | **Alta** (×6) | **Media** (×4) | **Baja** (×2)
> Convención de impacto/probabilidad: Alto (×3) | Medio (×2) | Bajo (×1)
> Severidad = Probabilidad × Impacto

---

## R1: Hooks de teclado no capturan eventos en producción
- **Categoría:** seguridad
- **Probabilidad:** Alta
- **Impacto:** Crítico
- **Severidad:** 9 (Crítica)
- **Estado:** Activo
- **Descripción:** Las implementaciones reales de `SetWindowsHookExW` (windows.rs:115-130), `CGEventTap` (macos.rs:185-235) y el grab de XCB (linux.rs:139-181) están comentadas o son stubs. El motor arranca un thread que solo duerme en un bucle. El binario compilado NO captura teclas reales — el flag de "aprobado" en phase4-review-final.md oculta este hecho crítico.
- **Mitigación concreta:**
  1. Descomentar y compilar el `unsafe extern "system" fn hook_proc` en `windows.rs` (líneas 120-134) y reemplazar el bucle `while !stop_flag` por un `GetMessageW`/`DispatchMessageW` real.
  2. En `linux.rs` añadir `xcb::xkb::use_extension` + `xcb_grab_keyboard` con `XCB_GRAB_MODE_ASYNC`.
  3. En `macos.rs` habilitar el `CGEventTap::new(...)` y crear un `CFRunLoop` activo.
  4. Añadir test de integración `tests/hook_integration.rs` que inyecte eventos sintéticos en cada plataforma.
  5. Bloquear release hasta que los tres hooks emitan un `HookEvent` real en `cargo run -- repl`.
- **Owner:** Engineer (Fase 4 cleanup)
- **Detectado en:** `src/hooks/{windows,linux,macos}.rs`

---

## R2: WH_KEYBOARD_LL (Windows) requiere elevación de privilegios
- **Categoría:** seguridad
- **Probabilidad:** Alta
- **Impacto:** Alto
- **Severidad:** 6 (Alta)
- **Estado:** Activo
- **Descripción:** Un hook `WH_KEYBOARD_LL` global requiere que el proceso corra como Administrador o con `SeDebugPrivilege`. Cuando un usuario sin privilegios ejecute el binario, la llamada a `SetWindowsHookExW` retorna `NULL` y el motor se vuelve un no-op silencioso. No hay detección ni mensaje de error user-friendly. Además, software antivirus/EDR puede marcar el binario como keylogger.
- **Mitigación concreta:**
  1. Detectar `GetLastError() == ERROR_ACCESS_DENIED` y emitir `HookError::PermissionDenied` antes de gastar el thread.
  2. Documentar en `README.md` que el modo `system` requiere admin; ofrecer modo `application` por defecto.
  3. Firmar el binario con un certificado EV (Extended Validation) para reducir falsos positivos de AV.
  4. Implementar un fallback automático: si el modo system falla, degradar a modo `application` y notificar al usuario.
- **Owner:** Security Engineer
- **Detectado en:** `src/hooks/windows.rs:130` (unsafe commented)

---

## R3: CGEventTap (macOS) requiere permisos de Accessibility
- **Categoría:** seguridad
- **Probabilidad:** Alta
- **Impacto:** Alto
- **Severidad:** 6 (Alta)
- **Estado:** Activo
- **Descripción:** `CGEventTap` solo funciona si el proceso está en la lista de **System Settings → Privacy & Security → Accessibility**. Sin ese permiso, el tap retorna `nil` y el hook falla silenciosamente. Adicionalmente, macOS puede revocar el permiso en cualquier momento, lo que requiere un watchdog para re-suscribirse. La notarización obligatoria para distribución fuera del App Store añade fricción.
- **Mitigación concreta:**
  1. Antes de crear el tap, llamar `AXIsProcessTrusted()` y si retorna `false`, mostrar un diálogo con instrucciones paso a paso para habilitar Accessibility.
  2. Implementar re-suscripción automática cuando se detecta que el tap murió.
  3. Notarizar el binario con `notarytool` antes de distribuir.
  4. Documentar el paso de habilitación en `README.md` y en `--help`.
- **Owner:** macOS Engineer
- **Detectado en:** `src/hooks/macos.rs:185-235` (CGEventTap commented)

---

## R4: Linux sin soporte para Wayland
- **Categoría:** seguridad / portabilidad
- **Probabilidad:** Alta
- **Impacto:** Alto
- **Severidad:** 6 (Alta)
- **Estado:** Aceptado (temporal)
- **Descripción:** El hook usa `xcb` (X11). Wayland, el compositor por defecto en Fedora 25+, Ubuntu 17.10+, Debian 10+, no permite captura global de teclas por diseño (modelo de seguridad). El binario falla silenciosamente cuando `DISPLAY` apunta a una sesión Wayland o no existe (típico en sesiones SSH, contenedores Docker, distros recientes).
- **Mitigación concreta:**
  1. Detectar protocolo: `echo $XDG_SESSION_TYPE` o `WAYLAND_DISPLAY`.
  2. Si es Wayland, registrar `HookError::PlatformError("Wayland not supported")` y sugerir al usuario cambiar a X11 (`XDG_SESSION_TYPE=x11`).
  3. Investigación a medio plazo: integración con `wlr-input-inhibitor` (Sway/Hyprland), `gnome-shell` extension API, o `ibus`/`fcitx` para captura a nivel de IME.
  4. Documentar en README: "Linux requiere X11 en esta versión".
- **Owner:** Linux Engineer
- **Detectado en:** `src/hooks/linux.rs:145-184`

---

## R5: log_keystrokes captura contraseñas y datos sensibles
- **Categoría:** seguridad / privacidad
- **Probabilidad:** Alta
- **Impacto:** Crítico
- **Severidad:** 9 (Crítica)
- **Estado:** Mitigado parcialmente
- **Descripción:** La flag `log_keystrokes: true` en `config.yaml:48` hace que todos los keystrokes capturados se persistan en logs de tracing. Si un usuario activa esto para debug, las contraseñas, números de tarjeta y datos EHR (PHI) se escribirán a disco. La documentación no advierte claramente sobre este riesgo. El `final-review-2026-06-16.md:64` lo marca como nota pero no como bloqueante.
- **Mitigación concreta:**
  1. Cambiar el default a `log_keystrokes: false` y requerir flag CLI explícito (`--log-keys`).
  2. Implementar filtro: cuando la ventana enfocada es un campo `type=password` (Windows: `EM_GETPASSWORDCHAR`; macOS: `isSecureTextField`; Linux: `wlr-input-method`), omitir el log.
  3. Sanitizar: en logs, ofuscar automáticamente caracteres de campos marcados como sensibles.
  4. Añadir warning en `config.yaml` y `README.md` sobre el riesgo legal (HIPAA, GDPR).
  5. Persistir `last_log_warning` acknowledged en el archivo de configuración para no repetir el aviso.
- **Owner:** Security Engineer + Compliance
- **Detectado en:** `config.yaml:48`, `src/hooks/platform.rs:75`

---

## R6: find_similar() es O(n) — degrada con diccionarios grandes
- **Categoría:** performance
- **Probabilidad:** Alta
- **Impacto:** Alto
- **Severidad:** 6 (Alta)
- **Estado:** Activo
- **Descripción:** `Trie::find_similar()` en `trie.rs:196-219` invoca `self.all_words()` que copia TODAS las palabras del trie a un `Vec<(String, u64)>`, después itera calculando Damerau-Levenshtein para cada una. Con un diccionario real de 100K palabras (inglés) y max_edit_distance=2, esto es ~10⁹ operaciones de Damerau por corrección. Documentado en issues-acumulados.md como P1-03 pero sin resolver.
- **Mitigación concreta:**
  1. Corto plazo: cachear `all_words()` con `OnceCell<Vec<(String, u64)>>` (la Trie es inmutable post-construcción).
  2. Mediano plazo: implementar búsqueda BK-tree o Symspell para reducir a O(log n) por corrección.
  3. Limitar `max_edit_distance` por longitud de palabra: distancia_max = min(configured, len(word)/2 + 1).
  4. Pre-filtrar candidatos por primera letra (no comparar "teh" contra palabras que empiezan por 'a').
  5. Benchmark: con 100K palabras, mantener p99 <5ms (objetivo actual: 50K <100ms en stress_test.rs:333).
- **Owner:** Performance Engineer
- **Detectado en:** `src/core/trie.rs:196-219`

---

## R7: Buffer overflow silencioso en el búfer de caracteres
- **Categoría:** ux
- **Probabilidad:** Alta
- **Impacto:** Medio
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** `CharBuffer::push()` en `buffer.rs:141-153` trunca silenciosamente cuando el búfer alcanza `MAX_BUFFER_SIZE = 64`. Adicionalmente hay un bug abierto (P1-02): el evento emitido es `BufferOverflowPrevented` aunque el carácter SÍ se haya agregado después del truncamiento. El usuario que escribe un URL de 100 caracteres ve cómo se pierden los primeros 36 sin notificación.
- **Mitigación concreta:**
  1. Corregir la nomenclatura: emitir `BufferTruncated` cuando se trunca, `BufferOverflowPrevented` solo si el char se descarta.
  2. Aumentar `MAX_BUFFER_SIZE` a 256 (palabra técnica más larga: "internationalization" = 20 chars, 256 cubre URLs simples).
  3. Implementar modo "no-truncate" con `VecDeque::push_back` y advertencia.
  4. Añadir listener en TypeFixPipeline que logge truncations a tracing::warn!.
  5. Resolver el issue P1-02 de issues-acumulados.md.
- **Owner:** Core Engineer
- **Detectado en:** `src/core/buffer.rs:141-153`

---

## R8: Damerau-Levenshtein con strings largas asigna ~1MB
- **Categoría:** performance / memoria
- **Probabilidad:** Media
- **Impacto:** Alto
- **Severidad:** 4 (Media)
- **Estado:** Mitigado parcialmente
- **Descripción:** `DamerauLevenshtein::distance_general()` en `damerau.rs:158-211` asigna una matriz de `(len1+1) × (len2+1)`. Con strings de 1000 chars son ~8MB (matriz usize). El pool de matrices está limitado a 4 (`damerau.rs:232`) — matrices más grandes se dropean. Si un atacante inyecta un keystroke de 10K chars (pegado masivo), la asignación bloquea.
- **Mitigación concreta:**
  1. Establecer un límite duro: si `len1 * len2 > 1_000_000`, retornar `max_dist + 1` sin calcular.
  2. Usar matriz de `u8` en lugar de `usize` para reducir memoria 8×.
  3. Implementar banda (banded matrix) para Damerau, explotando que la mayoría de correcciones están a distancia ≤2.
  4. Resolver el issue P3-02 de issues-acumulados.md.
- **Owner:** Algorithm Engineer
- **Detectado en:** `src/correction/damerau.rs:158-211`, P3-02

---

## R9: Vec::remove(0) en word_window es O(n)
- **Categoría:** performance
- **Probabilidad:** Alta
- **Impacto:** Bajo
- **Severidad:** 2 (Baja)
- **Estado:** Activo
- **Descripción:** `LanguageDetector::process_word()` en `detector.rs:108-114` hace `window.remove(0)` cuando excede `window_size`. Esto desplaza todos los elementos restantes — O(n) por palabra. Con 100 palabras por minuto y `window_size=5` es despreciable, pero en sesiones intensivas (EHR con paste) podría notarse. Documentado como P2-01.
- **Mitigación concreta:**
  1. Reemplazar `Vec<String>` con `VecDeque<String>` (deque es O(1) para pop_front).
  2. Alternativa: buffer circular con índice `head % capacity`.
  3. Benchmark: con 1000 palabras, mantener <10µs por push.
- **Owner:** Performance Engineer
- **Detectado en:** `src/language/detector.rs:108-114`, P2-01

---

## R10: Diccionarios de prueba (70-114 palabras), no producción
- **Categoría:** idiomas / ux
- **Probabilidad:** Alta
- **Impacto:** Crítico
- **Severidad:** 9 (Crítica)
- **Estado:** Activo
- **Descripción:** Los archivos `data/dictionaries/{en,es,pt}.json` contienen 114/114/72 palabras — son ejemplos, no diccionarios reales. Un corrector con 114 palabras solo corregirá "teh"→"the" porque están hardcodeados. El README menciona "high-density text input" pero la realidad es que cualquier palabra fuera del set de 114 se reporta como "no encontrada" → corrección incorrecta o ausente. El target es EHR/legal donde el vocabulario es de ~50K términos técnicos.
- **Mitigación concreta:**
  1. Corto plazo: importar lista de palabras de alta frecuencia de un corpus público (por ejemplo, top 50K de OpenSubtitles, Wikipedia, o Brown Corpus para en/es/pt).
  2. Generar JSON desde un script Python reproducible (`scripts/build_dictionary.py`).
  3. Mediano plazo: soportar Trie binario serializado (formato `.trie`) para reducir tamaño en disco 5-10× vs JSON.
  4. Documentar el tamaño mínimo aceptable (≥10K palabras) en `README.md` antes de la 1.0.
  5. Añadir métrica de "vocabulary coverage" en los benchmarks de stress_test.rs.
- **Owner:** Data Engineer
- **Detectado en:** `data/dictionaries/*.json` (medidos: 114, 114, 72 palabras)

---

## R11: Português sin mapa de errores estáticos
- **Categoría:** idiomas
- **Probabilidad:** Alta
- **Impacto:** Medio
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** `data/errors/` solo contiene `en.json` y `es.json`; no hay `pt.json`. El `config.yaml:10-13` lista "pt" como idioma soportado, pero el `init()` en `lib.rs:78-82` carga el mapa de errores sin fallar si no existe (chequea `if errors_path.exists()` en `lib.rs:111-113`). El usuario portugués obtiene corrección de menor calidad: solo Damerau-Levenshtein, sin el atajo O(1) de los errores frecuentes.
- **Mitigación concreta:**
  1. Crear `data/errors/pt.json` con al menos los 50 errores más comunes (analogos a "teh"→"the", "qeu"→"que").
  2. Mejor: el `init()` debe hacer fail-fast si `supported_languages` referencia un idioma sin datos.
  3. Documentar en config.yaml: para habilitar un idioma nuevo se requiere `dictionaries/X.json`, `stopwords/X.json` y (opcional) `errors/X.json`.
- **Owner:** Localization Engineer
- **Detectado en:** `data/errors/` (falta `pt.json`), `src/lib.rs:111-113`

---

## R12: Falsos positivos en corrección (distance=1 muy permisivo)
- **Categoría:** ux
- **Probabilidad:** Alta
- **Impacto:** Alto
- **Severidad:** 6 (Alta)
- **Estado:** Activo
- **Descripción:** `EngineConfig::default()` en `engine.rs:62-71` usa `max_edit_distance: 1`. Esto significa que "teh"→"the" funciona, pero también "wont"→"want" (sustitución) o "form"→"from" (transposición). En texto legal/EHR, un cambio de palabra puede alterar el significado: "patient has form" vs "patient has from" son clínica y legalmente distintos. No hay forma de "reject" la corrección, ni undo.
- **Mitigación concreta:**
  1. Implementar confidence threshold: solo corregir si la frecuencia del candidato es >10× la frecuencia de la palabra original.
  2. Modo `suggestion_mode: true` por defecto en producción: mostrar corrección sin aplicar.
  3. Si auto_correct, registrar todas las correcciones en un log de auditoría (con undo).
  4. Reglas de dominio: listas de palabras protegidas (nombres propios, medicamentos, códigos ICD-10) que nunca se autocorrigen.
  5. Prompt al usuario la primera vez que se detecta una corrección de alto riesgo.
- **Owner:** UX Engineer
- **Detectado en:** `src/correction/engine.rs:62-71`

---

## R13: Lenguaje de detección puede oscilar cerca del threshold
- **Categoría:** ux / idiomas
- **Probabilidad:** Media
- **Impacto:** Alto
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** `LanguageDetector::should_switch_language()` en `detector.rs:183-201` usa `confidence_threshold: 0.85` y `hysteresis_zone: 0.10`. En code-switching (mezcla de idiomas en un párrafo), el detector puede alternar entre en/es en cada ventana. La hysteresis_zone mitiga esto solo parcialmente. El test `test_hysteresis` en detector.rs:365 valida el comportamiento, pero no cubre oscilaciones rápidas.
- **Mitigación concreta:**
  1. Aumentar `min_words_before_switch` de 5 a 10 para reducir switches.
  2. Implementar "stickiness": requerir que la nueva lengua gane N ventanas consecutivas antes de cambiar.
  3. Logging: registrar cada switch con `tracing::warn!` para diagnosticar.
  4. Modo "manual language lock": CLI flag `--lang=es` para desactivar la detección.
  5. Análisis de oscilación: añadir test de integración que envíe 1000 palabras mezcladas y cuente switches.
- **Owner:** Language Engineer
- **Detectado en:** `src/language/detector.rs:183-201`

---

## R14: No hay soporte para CJK, RTL, ni segmentación de palabras
- **Categoría:** idiomas
- **Probabilidad:** Alta
- **Impacto:** Alto
- **Severidad:** 6 (Alta)
- **Estado:** Aceptado
- **Descripción:** `CharBuffer::is_word_char()` en `buffer.rs:253-255` solo considera alfanuméricos, apóstrofo y guion. Para japonés/chino (sin espacios), todo el texto se acumula en el buffer hasta encontrar puntuación — un párrafo completo se trata como una "palabra" de 200+ chars. Para árabe/hebreo, la dirección RTL se ignora. El Trie soporta Unicode (`trie.rs:453-462`) pero el buffer upstream no segmenta.
- **Mitigación concreta:**
  1. Corto plazo: usar `unicode-segmentation` (ya en Cargo.toml:38) en el buffer para delimitar palabras por grapheme clusters.
  2. Para CJK: usar `jieba-rs` o `tantivy-segmenter` para tokenización.
  3. Para RTL: usar `bidi` o `unicode-bidi` para detectar dirección y procesar correctamente.
  4. Documentar en README: "CJK y RTL no soportados en v0.1.0".
  5. Roadmap: v0.2.0 añadir japonés, v0.3.0 añadir chino, v0.4.0 añadir árabe.
- **Owner:** i18n Engineer
- **Detectado en:** `src/core/buffer.rs:253-255`, `Cargo.toml:38`

---

## R15: panic = "abort" en release impide recuperación
- **Categoría:** memoria / seguridad
- **Probabilidad:** Baja
- **Impacto:** Crítico
- **Severidad:** 3 (Media)
- **Estado:** Aceptado
- **Descripción:** `Cargo.toml:80` configura `panic = "abort"`. Cualquier panic en un thread de hook (windows.rs, linux.rs) mata el proceso completo, no solo el thread. Esto significa que un bug en el callback del hook puede tumbar el daemon en producción sin stack trace (no hay core dump por default en Windows). El `final-review-2026-06-16.md:100` declara "0 crashes" pero esto es engañoso porque abort = crash.
- **Mitigación concreta:**
  1. Corto plazo: cambiar a `panic = "unwind"` en release y envolver los callbacks de hook en `std::panic::catch_unwind`.
  2. Mediano plazo: separar el daemon en dos procesos — uno principal (corrección) y uno de hook (captura) — comunicados por IPC.
  3. Implementar supervisor que reinicie el proceso de hook si muere.
  4. Habilitar core dumps (`ulimit -c unlimited`) en scripts de deployment.
- **Owner:** Reliability Engineer
- **Detectado en:** `Cargo.toml:80`, `src/hooks/*.rs` (callbacks sin catch_unwind)

---

## R16: Riesgo de deadlock en listener callbacks
- **Categoría:** concurrencia
- **Probabilidad:** Media
- **Impacto:** Alto
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** `CharBuffer::notify_listeners()` en `buffer.rs:247-253` invoca callbacks mientras sostiene `self.inner.read()`. Si un callback intenta llamar a `buffer.push()` (caso real: un callback que quiere reprocesar el char), el `push()` intentará adquirir `write()` mientras otro thread sostiene `read()` → **deadlock** (con `parking_lot::RwLock` es non-fair). El mismo patrón existe en `pipeline.rs:192-199`. El `final-review-2026-06-16.md:97` declara "sin mutex anidados" pero los listeners son callbacks anónimos sin contrato claro.
- **Mitigación concreta:**
  1. Clonar la lista de listeners antes de invocar, soltar el lock, después iterar.
  2. Documentar el contrato: "listeners must not call back into the buffer/pipeline".
  3. Añadir `assert!(!is_locked())` en debug builds.
  4. Considerar cambiar a `mpsc` channel en lugar de callbacks directos.
  5. Resolver el issue P1-02 (relacionado) y añadir tests de regresión.
- **Owner:** Concurrency Engineer
- **Detectado en:** `src/core/buffer.rs:247-253`, `src/pipeline.rs:192-199`

---

## R17: User errors HashMap crece sin límite
- **Categoría:** memoria
- **Probabilidad:** Media
- **Impacto:** Alto
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** `StaticErrorMap::learn()` en `static_map.rs:106-124` inserta en `inner.user_errors` cada vez que se llama. No hay eviction. En una sesión de 8 horas, un usuario activo podría aprender miles de errores personalizados. El `learn_from_user: true` está por default en `Cargo.toml` y `config.yaml:40`. El `save_user_errors` persiste a disco sin límite de tamaño.
- **Mitigación concreta:**
  1. Implementar LRU eviction: mantener solo los top-N (ej. 1000) errores aprendidos por frecuencia.
  2. Limitar tamaño de `user_errors.json` a 1MB (análogo a `MAX_JSON_SIZE` en `static_map.rs:42`).
  3. Añadir timestamp `last_used` y purgar entradas no usadas en N días.
  4. Comando CLI `typefix stats` para ver tamaño del mapa.
  5. Default seguro: `learn_from_user: false` en producción; requerir opt-in.
- **Owner:** Memory Engineer
- **Detectado en:** `src/correction/static_map.rs:106-124`

---

## R18: Hook receiver() panic en producción
- **Categoría:** concurrencia / ux
- **Probabilidad:** Alta
- **Impacto:** Crítico
- **Severidad:** 9 (Crítica)
- **Estado:** Activo
- **Descripción:** Las implementaciones reales de `WindowsHook::receiver()`, `LinuxHook::receiver()` y `MacOSHook::receiver()` hacen `panic!("receiver() called - not implemented in this skeleton")` (windows.rs:178-180, linux.rs:218-221, macos.rs:281-284). Cualquier consumidor que llame a `hook.receiver()` crashea el proceso instantáneamente. Combinado con `panic = "abort"`, esto es un crash inmediato. Solo MockHook implementa `receiver()` correctamente (`platform.rs:201-203`).
- **Mitigación concreta:**
  1. Hacer que el `Sender` creado en `start()` (windows.rs:101, linux.rs:123, macos.rs:162) se exponga vía `receiver()`.
  2. Cambiar el trait para que `start()` retorne `(JoinHandle, Receiver<HookEvent>)` o un `HookHandle` que contenga ambos.
  3. Eliminar los `panic!` reemplazándolos por `unimplemented!()` con un mensaje claro y un test que valide.
  4. Cambiar `panic = "abort"` a `"unwind"` para que el panic sea recuperable.
  5. Bloquear release hasta que `hook.receiver()` retorne un receiver funcional en las tres plataformas.
- **Owner:** Hooks Engineer
- **Detectado en:** `src/hooks/windows.rs:178-180`, `linux.rs:218-221`, `macos.rs:281-284`

---

## R19: Auto-correct no distingue entre texto y código
- **Categoría:** ux
- **Probabilidad:** Alta
- **Impacto:** Alto
- **Severidad:** 6 (Alta)
- **Estado:** Activo
- **Descripción:** El motor autocorrige sin saber en qué tipo de campo está. Si un desarrollador escribe código en un IDE (VSCode, IntelliJ) y el typefix está activo, los identificadores se "corrigen": `usrname`→`username` rompe el código. Lo mismo en passwords, URLs, identificadores EHR (códigos ICD-10 como `E11.9` no deben tocarse), y comandos CLI. La `EngineConfig` no tiene un flag `respect_identifiers`.
- **Mitigación concreta:**
  1. Añadir heurística: si la palabra contiene guión bajo, punto, números, o está en camelCase, NO corregir.
  2. Integración con ventanas: detectar tipo de campo (password, code editor) via Win32 `GetClassName` / macOS `AXUIElement`.
  3. Whitelist de patrones regex: `^\d+(\.\d+)*$` (códigos), `[A-Z]{2,}_[A-Z_]+` (constantes), `^[a-z]+[A-Z]` (camelCase).
  4. Modo por defecto: `suggestion_mode: true` (no auto-aplicar).
  5. Hotkey global para toggle on/off: `Ctrl+Shift+T` (similar a CapsLock).
- **Owner:** UX Engineer
- **Detectado en:** `src/pipeline.rs:42-50`, `src/correction/engine.rs:130-188`

---

## R20: Cross-compilation sin cross-compile.md automatizado
- **Categoría:** portabilidad
- **Probabilidad:** Alta
- **Impacto:** Medio
- **Severidad:** 4 (Media)
- **Estado:** Mitigado
- **Descripción:** `docs/cross-compile.md` documenta los targets pero el `.cargo/config.toml:1-27` solo configura `rustflags` (no toolchain ni linker). Compilar `x86_64-unknown-linux-musl` desde Windows requiere `rustup target add` y un linker musl (no incluido en Windows MSVC). El plan original (plan-implementacion.md:483) menciona "Docker build containers" pero no se ha implementado.
- **Mitigación concreta:**
  1. Crear `Dockerfile` con `cargo build --release --target x86_64-unknown-linux-musl`.
  2. Script `scripts/build-release.sh` con `--target` parametrizable.
  3. CI (GitHub Actions matrix): linux, macos, windows builds automáticos.
  4. Documentar prerrequisitos en `docs/cross-compile.md` (rustup targets, musl-tools, osxcross).
  5. Publicar binarios en GitHub Releases con SHA256 checksums.
- **Owner:** DevOps Engineer
- **Detectado en:** `.cargo/config.toml`, `docs/cross-compile.md`

---

## R21: Detección de lenguaje no usa priors reales
- **Categoría:** idiomas
- **Probabilidad:** Alta
- **Impacto:** Bajo
- **Severidad:** 2 (Baja)
- **Estado:** Activo
- **Descripción:** `LanguageDetector::add_language()` en `detector.rs:76-89` asigna priors uniformes: `1.0 / count as f64`. Esto asume que en/es/pt son equiprobables en el texto, lo cual es raramente cierto (en un texto legal mexicano, español debería tener prior >80%). Documentado como P2-02.
- **Mitigación concreta:**
  1. Cargar priors desde un archivo `data/priors.json` basado en corpus (por ejemplo, frecuencias de Wikipedia por país).
  2. Configuración por usuario: `default_language: "es"` con prior 0.8.
  3. Ajuste dinámico: si después de N ventanas el score de un idioma supera 0.5, ajustar el prior.
  4. Tests: validar que en un texto 100% en español, el detector retorne "es" con confidence >0.9.
- **Owner:** Data Engineer
- **Detectado en:** `src/language/detector.rs:76-89`, P2-02

---

## R22: process_string puede bloquear con texto muy largo
- **Categoría:** performance
- **Probabilidad:** Media
- **Impacto:** Medio
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** `TypeFixPipeline::process_string()` en `pipeline.rs:146-166` itera carácter por carácter. Para un texto de 10K chars (párrafo típico) el procesamiento toma ~10ms (Damerau por palabra). Para un paste de 1M chars (libro entero), el bloqueo puede ser >1s, congelando el UI. El buffer trunca a 64 chars (`MAX_BUFFER_SIZE`), pero `process_string` no tiene esta protección.
- **Mitigación concreta:**
  1. Procesar en chunks: yield al event loop cada N palabras.
  2. Usar `tokio::spawn` para `process_string` async.
  3. Límite duro: rechazar `process_string` con >100K chars y emitir warning.
  4. Métrica: loggear tiempo de procesamiento por cada 1000 palabras.
  5. Probar con `criterion` bench: `benches/bench_process.rs`.
- **Owner:** Performance Engineer
- **Detectado en:** `src/pipeline.rs:146-166`

---

## R23: Detección de UTF-8 corrupto silenciosa
- **Categoría:** seguridad / ux
- **Probabilidad:** Media
- **Impacto:** Medio
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** El plan original (plan-implementacion.md:486) menciona "Validación exhaustiva en boundary" para UTF-8 corrupto, pero el código no la implementa. `Trie::insert()` (trie.rs:68-83) acepta cualquier `&str` Rust válido. Si un archivo JSON de diccionario tiene UTF-8 malformado, `serde_json::from_str` retorna error, pero el `init()` continúa con los idiomas que sí cargaron. No hay validación de los strings insertados en el Trie.
- **Mitigación concreta:**
  1. Validar cada palabra en `Trie::insert()`: debe pasar `unicode-normalization` (NFC) y `unicode-segmentation` (no ser combinación vacía).
  2. Validar JSON: rechazar caracteres de control, private use area, replacement char (U+FFFD).
  3. Logger: en `init()`, si un idioma falla, hacer `tracing::error!` y abortar (no continuar a medias).
  4. Tests: insertar palabras con BOM, RTL marks, zero-width joiner; verificar rechazo o normalización.
- **Owner:** Security Engineer
- **Detectado en:** `src/core/trie.rs:68-83`, plan-implementacion.md:486

---

## R24: macOS hook usa OnceLock + Mutex global (anti-pattern)
- **Categoría:** concurrencia
- **Probabilidad:** Baja
- **Impacto:** Alto
- **Severidad:** 2 (Baja)
- **Estado:** Activo
- **Descripción:** `macos.rs:38-40` define `EVENT_SENDER` y `LOG_KEYSTROKES` como `OnceLock<Arc<Mutex<...>>>`. Esto es un global mutable compartido entre todos los `MacOSHook` instanciados. Si dos procesos de typefix corren simultáneamente, comparten el sender. Además, el callback del `CGEventTap` (línea 218) accede a `LOG_KEYSTROKES.get().map(...)` que puede ser `None` si el `start()` aún no terminó, perdiendo eventos.
- **Mitigación concreta:**
  1. Pasar el `Sender` y el `log_keystrokes` por closure al `CGEventTap`, no por global.
  2. Eliminar los `OnceLock` — usar `Arc<MacOSHookShared>` pasado a cada instancia.
  3. Sincronizar: el callback debe esperar a que `start()` complete la inicialización (`AtomicBool ready`).
  4. Documentar que solo puede haber UN `MacOSHook` activo por proceso.
- **Owner:** macOS Engineer
- **Detectado en:** `src/hooks/macos.rs:38-40, 218`

---

## R25: No hay rate limiting en API pública
- **Categoría:** seguridad / ux
- **Probabilidad:** Media
- **Impacto:** Alto
- **Severidad:** 4 (Media)
- **Estado:** Activo
- **Descripción:** El CLI `typefix repl` y `typefix correct` no tienen rate limiting. Un script malicioso podría llamar `correct()` 10K veces/segundo consumiendo CPU. El `correct_word` en `main.rs:164-188` crea un `Trie` nuevo en cada llamada, asignando memoria innecesaria.
- **Mitigación concreta:**
  1. CLI: token bucket de 1000 ops/seg con `governor` crate.
  2. Cachear el `Trie` en CLI: lazy_static o once_cell para reusar.
  3. Añadir timeout a `correct()`: si toma >100ms, abortar y retornar el original.
  4. Métricas: contar ops/seg y loggear picos.
- **Owner:** API Engineer
- **Detectado en:** `src/main.rs:164-188`, `src/pipeline.rs:63-205`

---

## Resumen por Severidad

| Severidad | Cuenta | Riesgos |
|-----------|--------|---------|
| Crítica (×9) | 4 | R1, R5, R10, R18 |
| Alta (×6) | 6 | R2, R3, R4, R6, R12, R14, R19 |
| Media (×4) | 11 | R7, R8, R11, R13, R15, R16, R17, R20, R22, R23, R25 |
| Baja (×2) | 3 | R9, R21, R24 |

## Resumen por Estado

| Estado | Cuenta |
|--------|--------|
| Activo | 22 |
| Mitigado parcialmente | 2 (R5, R8) |
| Aceptado (temporal) | 3 (R4, R14, R15) |

## Resumen por Categoría

| Categoría | Cuenta | Riesgos |
|-----------|--------|---------|
| seguridad | 5 | R1, R2, R3, R4, R5, R23 |
| performance | 5 | R6, R8, R9, R22, R25 |
| memoria | 2 | R17, R15 (parcial) |
| concurrencia | 3 | R16, R18, R24 |
| ux | 5 | R7, R12, R13, R19, R25 |
| idiomas | 5 | R10, R11, R14, R21, R13 |
| portabilidad | 2 | R4, R20 |

---

*Risk register generado: 2026-06-16 — Revisar mensualmente hasta v1.0*
