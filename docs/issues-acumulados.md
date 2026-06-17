# TypeFix - Issues Acumulados

**Última actualización:** 2026-06-16  
**Total issues:** 12  
**Estado:** Pendientes de resolver en fase "cleanup"

---

## Issues por Fase

### Phase 1: Core Modules

| # | Severidad | Módulo | Descripción | Líneas |
|---|-----------|--------|-------------|--------|
| P1-01 | ⚠️ Minor | trie.rs | `word_window.clone()` ineficiente en detector.rs - hace clone de todo el vec en cada palabra | detector.rs:100 |
| P1-02 | ⚠️ Minor | buffer.rs | Nomenclatura confusa: `BufferOverflowPrevented` se emite cuando el char SÍ se agregó (después del truncate) | buffer.rs:151 |
| P1-03 | ⚠️ Minor | trie.rs | `find_similar()` itera sobre TODAS las palabras - puede ser lento con diccionarios grandes | trie.rs:194-217 |
| P1-04 | ⚠️ Minor | detector.rs | `unwrap()` en max_by - contexto seguro pero podría fallar con HashMap vacío | detector.rs:119 |

### Phase 2: Language Detection

| # | Severidad | Módulo | Descripción | Líneas |
|---|-----------|--------|-------------|--------|
| P2-01 | 🔧 Suggestion | detector.rs | Usar circular buffer en lugar de `Vec::remove(0)` para mejor performance | detector.rs:105 |
| P2-02 | 🔧 Suggestion | detector.rs | Considerar implementar priors personalizados (no uniformes) | detector.rs:72-78 |

### Phase 3: Correction Engine

| # | Severidad | Módulo | Descripción | Líneas |
|---|-----------|--------|-------------|--------|
| P3-01 | ⚠️ Minor | pipeline.rs | Warnings de compilación: variables `_r1`, `_r2` no usadas en tests | pipeline.rs:280-282 |
| P3-02 | ⚠️ Minor | damerau.rs | Strings >1000 chars allocan ~1MB para matrix - aceptable para palabras pero no para texto largo | damerau.rs:242 |
| P3-03 | 🔧 Suggestion | engine.rs | `from_json` usa `Box<dyn Error>` - considerar error type más específico | engine.rs |

### Phase 4: OS Hooks (Implementado)

| # | Severidad | Módulo | Descripción | Líneas | Estado |
|---|-----------|--------|-------------|--------|--------|
| P4-01 | ⚠️ Minor | windows.rs | Hook funcional pero unsafe comentado - requiere testing real | windows.rs | Partial |
| P4-02 | ⚠️ Minor | linux.rs | Hook conecta a XCB y hace polling | linux.rs | Partial |
| P4-03 | ⚠️ Minor | macos.rs | CGEventTap comentado - estructura lista | macos.rs | Partial |
| P4-04 | ✅ Fixed | platform.rs | Tests agregados, MockHook funciona correctamente | platform.rs | Fixed |

---

## Issues Pendientes de Resolver

### 🔴 Critical (3)
- P4-01: Windows hook stub
- P4-02: Linux hook stub
- P4-03: macOS hook stub

### ⚠️ Minor (7)
- P1-01: word_window.clone()
- P1-02: BufferOverflowPrevented nomenclatura
- P1-03: find_similar() performance
- P1-04: unwrap() en max_by
- P3-01: Warnings de compilación
- P3-02: Damerau memory allocation
- P4-04: MockHook unwrap()

### 🔧 Suggestion (4)
- P2-01: Circular buffer
- P2-02: Custom priors
- P3-03: Error type específico

---

## Plan de Resolución

**Fase "Cleanup":** Después de completar Phase 4 y su review, resolver todos los issues menores y suggestions.

**Orden sugerido:**
1. P4-01, P4-02, P4-03 → Phase 4 (Priority Critical)
2. P1-02, P1-04, P4-04 → Cleanup (fácil)
3. P2-01, P1-03 → Cleanup (refactor)
4. P1-01, P3-02 → Cleanup (optimization)
5. P3-01 → Cleanup (compilation warnings)
6. P2-02, P3-03 → Cleanup (nice-to-have)

---

*Issues tracking creado: 2026-06-16*
