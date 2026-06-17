# Plan de Implementación: Core Typo Correction & Language Detection Engine

> **Estado del proyecto:** Planificacion  
> **Nombre:** Pendiente de definir  
> **Versión del plan:** 1.1  
> **Fecha:** 2026-06-16  
> **Ultima actualizacion:** 2026-06-16 (requisitos de testing, isolation y governance)

---

## 1. Resumen Ejecutivo

Motor de correccion de typos y deteccion de idioma en tiempo real, escrito en Rust. Diseñado para entornos de alto volumen como EHR medico y documentacion legal. Objetivo: latencia ~0ms, huella de memoria <10MB RAM, sin garbage collection.

---

## 2. Fases de Implementacion

### Fase 1: Fundamentos del Motor (Semanas 1-3)

**Objetivo:** Establecer el nucleo del sistema con la estructura de datos central y los componentes basicos.

#### 1.1 Configuracion del Proyecto
- [ ] Inicializar proyecto Rust con Cargo
- [ ] Configurar estructura de modulos:
  ```
  src/
    ├── core/           # Nucleo del motor
    │   ├── buffer.rs   # Gestor de buffer de caracteres
    │   ├── trie.rs     # Estructura Trie para diccionarios
    │   └── config.rs   # Configuracion centralizada
    ├── language/       # Deteccion de idioma
    │   ├── detector.rs # Motor Bayesiano
    │   └── stopwords.rs# Trie de stopwords
    ├── correction/     # Correccion de typos
    │   ├── damerau.rs  # Distancia Damerau-Levenshtein
    │   ├── static_map.rs# Mapa de errores frecuentes
    │   └── engine.rs   # Motor de correccion
    ├── hooks/          # Integracion con SO
    │   ├── windows.rs  # Hooks de teclado Windows
    │   └── linux.rs    # Hooks para Linux
    └── lib.rs          # Punto de entrada de la libreria
  ```
- [ ] Configurar logging con `tracing`
- [ ] Definir estructura de configuracion (TOML/YAML)

#### 1.2 Implementacion del Trie
- [ ] Implementar Trie generico en `src/core/trie.rs`
- [ ] Soporte para Unicode completo
- [ ] Metodos: `insert`, `search`, `starts_with`, `get_all_with_prefix`
- [ ] Tests unitarios con coverage >90%

#### 1.3 Buffer de Caracteres
- [ ] Implementar `RingBuffer` con tamanio maximo configurable (default: 64 chars)
- [ ] Thread-safety con Mutex/RwLock
- [ ] Deteccion de delimitadores: espacio, Enter, Tab, puntuacion
- [ ] Eventos de buffer lleno para manejo de strings largos

#### 1.4 Carga de Datos Inicial
- [ ] Formato de diccionario: JSON con estructura `{"palabra": ["forma1", "forma2"]}`
- [ ] Loader de diccionarios con lazy loading
- [ ] Verificar tamanios de memoria por idioma:
  - Espanol: ~50K palabras base
  - Ingles: ~50K palabras base
  - Portugues: ~50K palabras base

**Criterio de aceptacion Fase 1:**
- `cargo test` pasa al 100%
- Benchmark: insercion de 50K palabras < 500ms
- Memoria base del motor < 2MB

---

### Fase 2: Deteccion de Idioma (Semanas 4-6)

**Objetivo:** Sistema de deteccion pasiva que cambia entre idiomas sin intervencion del usuario.

#### 2.1 Stopwords Trie por Idioma
- [ ] Crear archivos de stopwords para cada idioma soportado:
  - `data/stopwords/es.json`
  - `data/stopwords/en.json`
  - `data/stopwords/pt.json`
- [ ] Implementar generador de Trie desde JSON
- [ ] Singleton de gesti\u00f3n de Trie activo

#### 2.2 Motor Bayesiano
- [ ] Implementar `BayesianDetector`:
  - Ventana movil de ultimas 3-5 palabras
  - Calculo de probabilidad P(idioma|palabra)
  - Scores para cada idioma activo
- [ ] Threshold de confianza: 85%
- [ ] Histéresis: no cambiar idioma si confidence entre 75-85% (zona de ambigüedad)
- [ ] Anti-oscilacion: minimo 5 palabras antes de re-evaluar

#### 2.3 Deteccion en Tiempo Real
- [ ] Integrar detector con buffer de entrada
- [ ] Pipeline: buffer -> tokenizacion -> evaluacion -> cambio de idioma
- [ ] Logueo de cambios de idioma para debugging

**Criterio de aceptacion Fase 2:**
- Precision de deteccion > 90% en textos de prueba
- Tiempo de evaluacion < 1ms por palabra
- Sin cambios de idioma espurios en textos de prueba

---

### Fase 3: Correccion de Typos (Semanas 7-10)

**Objetivo:** Motor de correccion con Damerau-Levenshtein + mapa estatico.

#### 3.1 Mapa Estatico de Errores
- [ ] Implementar `StaticErrorMap` con HashMap precalculado
- [ ] Loader de errores frecuentes desde JSON:
  - `data/errors/es.json` (errores comunes en espanol)
  - `data/errors/en.json`
  - `data/errors/pt.json`
- [ ] O(1) lookup para errores conocidos
- [ ] Extension runtime: guardar errores del usuario

#### 3.2 Damerau-Levenshtein Distance = 1
- [ ] Implementar `damerau_levenshtein` en `src/correction/damerau.rs`
- [ ] Optimizado para distance = 1 (caso mas comun)
- [ ] Solo transposiciones adyacentes (el typo mas comun)
- [ ] Matrix pooling para evitar allocaciones

#### 3.3 Motor de Correccion
- [ ] `CorrectionEngine` con pipeline:
  1. Check mapa estatico (O(1))
  2. Si no match: Damerau-Levenshtein distance = 1
  3. Seleccionar candidato con mayor frecuencia en diccionario
- [ ] Multiple candidatos: devolver el mas probable
- [ ] Sin candidatos validos: devolver original

#### 3.4 Retroalimentacion del Usuario
- [ ] API para marcar correccion como correcta/incorrecta
- [ ] Actualizacion del mapa estatico en tiempo de ejecucion
- [ ] Persistencia de aprendizaje en archivo local

**Criterio de aceptacion Fase 3:**
- Correccion de transposicion mas comun ("qeu" -> "que") < 0.1ms
- Precision de correccion > 85% en dataset de prueba
- Sin correccion excesiva (false positives < 5%)

---

### Fase 4: Integracion con Sistema Operativo (Semanas 11-13)

**Objetivo:** Hooks de teclado para captura y sustitucion transparente.

#### 4.1 Windows Integration
- [ ] Implementar `WindowsHook` usando `winapi` crate
- [ ] Low-level keyboard hook (WH_KEYBOARD_LL)
- [ ] Inyeccion de caracteres corregidos
- [ ] Manejo de estados: Normal, Caps Lock, Shift
- [ ] Testing en Windows 10/11

#### 4.2 Linux Integration
- [ ] Implementar `LinuxHook` usando `xcb` o `libinput`
- [ ] X11 y Wayland support
- [ ] Inyeccion de eventos de teclado

#### 4.3 macOS Integration
- [ ] Implementar `MacOSHook` usando `cocoa` y `core-graphics`
- [ ] CGEvent tap para captura
- [ ] CGEvent post para inyeccion

#### 4.4 Servicio/Daemon
- [ ] Wrapper como servicio Windows (nssm/winsw)
- [ ] Demonio Linux (systemd)
- [ ] LaunchAgent macOS
- [ ] Comunicacion IPC: Unix socket o named pipe

**Criterio de aceptacion Fase 4:**
- Latencia de correccion end-to-end < 5ms
- 0 caracteres perdidos o duplicados
- Funciona en background sin afectar otras apps

---

### Fase 5: Optimizacion y Testing de Rendimiento (Semanas 14-16)

**Objetivo:** Validar que cumple con los requisitos de produccion.

#### 5.1 Benchmarking
- [ ] Memoria base del motor (idle)
- [ ] Latencia por correccion
- [ ] Throughput: correcciones por segundo
- [ ] Uso de CPU en idle y bajo carga

#### 5.2 Memory Profiling
- [ ] Heap allocations en caliente
- [ ] Reduccion de uso de memoria < 10MB
- [ ] Pooling de objetos frecuentes

#### 5.3 Stress Testing
- [ ] 10,000+ correcciones por minuto
- [ ] Cambio rapido de idiomas (es/en/pt)
- [ ] Strings sin espacios > 100 caracteres

#### 5.4 Testing de Integracion
- [ ] Test con editores de texto reales
- [ ] Test con browsers
- [ ] Test con aplicaciones medicas/legalessimuladas

**Criterio de aceptacion Fase 5:**
- Memoria idle < 10MB
- Latencia media < 1ms
- Sin memory leaks en 24h de uso continuo

---

### Fase 6: Despliegue y Documentacion (Semanas 17-18)

**Objetivo:** Entrega lista para produccion.

#### 6.1 Build y Distribucion
- [ ] Cross-compilation para Windows (.exe + .dll)
- [ ] Linux binaries (x86_64, ARM)
- [ ] macOS binaries (Intel + Apple Silicon)
- [ ] Size optimization (strip, LTO)

#### 6.2 Documentacion Tecnica
- [ ] README.md con arquitectura y usage
- [ ] API documentation (rustdoc)
- [ ] Diagrama de arquitectura (Mermaid)
- [ ] Gu\u00eda de integraci\u00f3n para EHR/Legal systems

#### 6.3 Documentacion de Usuario
- [ ] Guia de instalacion rapida
- [ ] Configuracion de idiomas
- [ ] FAQ y troubleshooting

---

## 3. Dependencias Rust Recomendadas

```toml
[dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"

# Platform hooks
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["winuser", "keyboardhook"] }

[target.'cfg(target_os = "linux")'.dependencies]
xcb = "1.14"

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"
core-graphics = "0.23"

# Utilities
once_cell = "1.18"
parking_lot = "0.12"
unicode-segmentation = "1.10"
```

---

## 4. Estructura de Datos para Diccionarios

### Formato de Diccionario Base
```json
{
  "language": "es",
  "version": "1.0",
  "words": [
    {"word": "que", "frequency": 1000000},
    {"word": "hola", "frequency": 500000}
  ]
}
```

### Formato de Stopwords
```json
{
  "language": "es",
  "stopwords": ["el", "la", "de", "que", "y"]
}
```

### Formato de Errores Frecuentes
```json
{
  "language": "es",
  "errors": {
    "qeu": "que",
    "qe": "que",
    "teh": "the"
  }
}
```

---

## 5. Metric and Acceptance Criteria Summary

| Metrica | Objetivo | Fase |
|---------|----------|------|
| Memoria idle | < 10MB | 5 |
| Latencia correccion | < 1ms | 3 |
| Latencia end-to-end | < 5ms | 4 |
| Precision deteccion idioma | > 90% | 2 |
| Precision correccion | > 85% | 3 |
| False positives | < 5% | 3 |
| Cobertura tests | > 90% | 1-3 |

---

## 6. Robust Error Isolation (Fail-Silent Design)

**Principio:** Cualquier fallo en el pipeline de correccion o diccionarios NO debe afectar la experiencia del usuario. El sistema debe degradarse gracefully.

### 6.1 Comportamiento ante Fallos
- [ ] **Buffer overflow:** Truncar y procesar solo los primeros 64 caracteres
- [ ] **Diccionario corrupto:** Devolver texto original sin crash
- [ ] **Trie no encontrado:** Usar fallback al idioma default
- [ ] **Lock contention timeout:** Skip correccion, devolver original
- [ ] **Thread panic:**捕获 con `catch_unwind`, devolver original

### 6.2 Implementacion Requerida
```rust
// Todo modulo debe implementar fail-safe:
fn correct_safe(&self, input: &str) -> String {
    let result = std::panic::catch_unwind(|| self.correct(input));
    match result {
        Ok(Ok(corrected)) => corrected,
        _ => input.to_string(), // Fail-safe: devolver original
    }
}
```

### 6.3 Criterio de Aceptacion
- [ ] **0 crashes** en testing con datos corruptos/invalidos
- [ ] **0 freezes** > 100ms ante fallos en componentes
- [ ] **100% graceful degradation:** siempre devuelve texto
- [ ] **Logueo de errores** para debugging sin afectar al usuario

---

## 7. Zero Shared Global State

**Principio:** Toda concurrencia usa estructuras inmutables o primitivas de sincronizacion seguras.

### 7.1 Politicas de Concurrencia
- [ ] **Arc\<RwLock\<T\>\>** para todos los datos compartidos mutables
- [ ] **Arc\<Trie\>** para diccionarios de solo lectura (clonacion economica)
- [ ] **Canal de mensajes** (mpsc) para comunicacion entre threads
- [ ] **NO Mutex para paths calientes** — usar RwLock con read-preference
- [ ] **NO Arc\<Mutex\<T\>\>** excepto para operations atomicas trivialmente cortas

### 7.2 Estructuras de Datos Inmutables
- [ ] **FrecDict:** HashMap congelado con `DashMap` o `RwLock<HashMap>`
- [ ] **Trie:** Inmutable post-carga, multiple readers simultaneos
- [ ] **Config:** `once_cell::sync::Lazy` para singleton thread-safe

### 7.3 Criterio de Aceptacion
- [ ] **0 data races** detectadas por ThreadSanitizer
- [ ] **0 deadlocks** en stress testing
- [ ] **Read throughput:** >100K palabras/segundo en lecturas concurrentes
- [ ] **Clippy:** `arc_with_raw_ptrs` y `mutex_atomic` checks passing

---

## 8. Comprehensive Boundary Testing

**Principio:** Todo input posible debe ser manejado, incluyendo edge cases extremos.

### 8.1 Memory Allocation Tests
- [ ] **Buffer maximo:** 64 chars — test con 65+ caracteres
- [ ] **Buffer minimo:** 0 caracteres — input vacio
- [ ] **Unicode maximo:** Strings de 10,000+ caracteres UTF-8
- [ ] **Memory leak detection:** 24h continuous operation con valgrind/asan

### 8.2 UTF-8 Edge Cases
- [ ] **Emojis:** "hola 😀 mundo 🌍" — no debe corromper buffer
- [ ] **Multi-byte scripts:** arabe, chino, ruso, japones
- [ ] **Combining characters:** e + combinar tilde = é (debe tratarse como 1 char)
- [ ] **Zero-width characters:** invisible chars que pueden romper metricas
- [ ] **BOM markers:** byte order marks en archivos UTF-16/UTF-32

### 8.3 Rapid-Fire Input Tests
- [ ] **Keyboard rollover:** 10+ teclas simultaneas
- [ ] **Burst input:** 100 caracteres/segundo continuo
- [ ] **Paste events:** 10KB+ de texto pegado de una vez
- [ ] **IME composition:** secuencias incompletas de input method

### 8.4 Criterio de Aceptacion
- [ ] **100% edge cases** cubiertos con tests
- [ ] **0 memory corruption** con inputs extremos
- [ ] **Latencia estable** bajo carga rapida

---

## 9. Multi-Agent Governance & CI/CD

**Principio:** Todo cambio de codigo requiere aprobacion unanime del committee.

### 9.1 Committee Structure

| Agente | Responsabilidad | Verdict |
|--------|-----------------|---------|
| **Agent-Architect** | DDD constraints, Rust low-level, memory management | APPROVED/REJECTED |
| **Agent-Developer** | Idiomatic Rust, clippy, algorithmic complexity | APPROVED/REJECTED |
| **Agent-QA** | Boundary conditions, error handling, no happy paths | APPROVED/REJECTED |
| **Agent-Security** | Unsafe blocks, memory leaks, OS hook vulnerabilities | APPROVED/REJECTED |

### 9.2 Pipeline CI/CD
- [ ] **Pre-commit hooks:**
  - `cargo fmt` + `cargo clippy` (warnings = error)
  - Unit tests con coverage >90%
  - Boundary tests contra edge cases UTF-8
- [ ] **Pull Request gates:**
  - Todos los agentesunanimemente APPROVED
  - 0 unwrap() sin comentario de justificacion
  - 0 FIXMEs/TODOs en codigo de produccion
- [ ] **Integration tests:**
  - Stress test con inputs extremos
  - Memory profiling con dhat/jemalloc
  - Latency profiling con Criterion

### 9.3 System Prompt para Code Review

```markdown
SYSTEM ROLE: Multi-Agent Code Review & Governance Committee
ENVIRONMENT: Production-Grade Rust Core Engine Integration

You are a highly collaborative, rigorous Multi-Agent Committee
consisting of an Architect, a Senior Developer, a Security Engineer,
and a QA Lead. Your collective objective is to audit incoming code
changes for a hyper-lightweight, zero-latency typo correction engine.

CRITICAL INSTRUCTIONS FOR THE COMMITTEE:
1. ABSOLUTE CONSENSUS: No pull request, code modification, or feature
   branch can be merged without unanimous approval from all four
   specialized agent roles.
2. NO SHORTCUTS: Reject any code that utilizes unhandled 'unwrap()'
   statements, assumes happy paths, lacks comprehensive edge-case
   handling for multi-byte UTF-8, or introduces bloated dependencies.
3. AGENT SKILL UTILIZATION: Leverage specialized tools to actively
   trace execution graphs, check performance overhead profiles, and
   execute structural validation tests.

INDIVIDUAL AGENT MANDATES:
- Architect: Enforce strict modular separation and zero-cost
  abstractions. Ensure FFI layers are memory-safe.
- Developer: Ensure idiomatic Rust (clippy compliant), optimal
  algorithmic complexity (O(1) mappings or O(N) bounded Tries),
  and DRY principles.
- QA Lead: Look for boundary conditions, buffer overflows, rapid
  hardware inputs, multi-language dictionary collision bugs, and
  missing unit/integration tests.
- Security: Audit unsafe blocks, pointer manipulations in hardware
  hooks, and OS level process privileges.

OUTPUT FORMAT:
Each agent must provide a distinct 'VERDICT: [APPROVED / REJECTED]'
accompanied by an explicit, technical analysis of their domain.
If any single agent rejects, the entire review process fails and
requires a rewrite.
```

### 9.4 Criterio de Aceptacion
- [ ] **100% PRs** pasan por committee review
- [ ] **0 unwrap()** en codigo de produccion
- [ ] **Coverage >90%** enforced en CI
- [ ] **0 security warnings** del Agent-Security

---

## 10. Riesgo y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Hooks de teclado bloquean input | Media | Alto | Testing intensivo, fallback a modo proxy |
| Memory leaks en runtime | Baja | Alto | Valgrind +asan en testing |
| Cambios de idioma espurios | Media | Medio | Threshold conservativo, hysteresis |
| Compatibilidad con IME | Media | Alto | Testing con teclado japones/chino |
| Cross-compilation compleja | Media | Medio | Docker build containers |
| Crash por panic no capturado | Baja | Critico | catch_unwind en todo modulo |
| Data race en Trie compartido | Baja | Critico | Arc<RwLock> everywhere |
| Inputs corruptos UTF-8 | Media | Alto | Validacion exhaustiva en边界 |

---

## 11. Siguiente Paso

**Nombre del proyecto:** Necesita ser definido por el usuario antes de proceder a Fase 1.

**Candidatos sugeridos:**
- TypeFix
- WordGuard
- TypeFlow
- CleanType
- ErrorZero

---

*Plan generado automaticamente - 2026-06-16*
