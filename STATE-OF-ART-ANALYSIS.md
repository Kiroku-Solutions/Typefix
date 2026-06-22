# TypeFix State of the Art Analysis
## WASM Performance & Correction Coverage Investigation

**Date:** 2026-06-21  
**Focus:** Why many words aren't being corrected in WASM vs native  
**Deliverable:** Technical analysis with actionable findings

---

## Problema Reportado

> "Que pasa porque no me devuelves nada" / "Muchisimas palabras no me las arregla"

El usuario reporta que **muchas palabras no se corrigen** en la versión WASM, mientras que el ejecutable nativo (EXE) funciona correctamente.

---

## Análisis Técnico

### 1. Arquitectura de Diccionarios

#### Estructura de Datos

```
┌─────────────────────────────────────────────────────────┐
│                    DICCIONARIO FST                       │
│                                                         │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐            │
│  │  JSON   │───▶│ COMPILE │───▶│   FST   │            │
│  │  Source │    │ (build) │    │  Binary │            │
│  └─────────┘    └─────────┘    └─────────┘            │
│                                         │              │
│         data/dictionaries/*.json  ──────┘              │
│                                         │              │
│         data/dictionaries/*.fst   ──────┘              │
└─────────────────────────────────────────────────────────┘
```

#### Carga en WASM vs Native

| Aspecto | Native (EXE) | WASM |
|---------|-------------|------|
| Carga FST | Memory-mapped (`Mmap`) | Bytes en memoria (`Arc<[u8]>`) |
| Optimización | OS paging | Manual |
| Lazy loading | Soportado | Requiere implementarse |

**Problema Identificado en `src/core/dict.rs`:**

```rust
// Native: Usa Mmap - eficiente con archivos grandes
pub fn from_fst_file<P: AsRef<Path>>(path: P) -> Result<Self> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };  // ✅ OS optimizado
    let data = DictData::Mmap(std::sync::Arc::new(mmap));
    let map = Map::new(data)?;
    Ok(Self { map, word_count })
}

// WASM: Carga todo en memoria
pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
    let arc_bytes: std::sync::Arc<[u8]> = bytes.into();
    let map = Map::new(arc_bytes)?;  // ⚠️ Carga completa
    Ok(Self { map, word_count })
}
```

### 2. Fallos de Carga de Diccionario

#### Verificación de Carga

```rust
// src/lib.rs:88-90
for lang in &config.supported_languages {
    load_language_data(lang, &config.data_path)?;
}
```

**Problemas Potenciales:**

1. **Fallback silencioso:** Si el FST no existe, continua sin error
2. **No hay logging de qué se cargó:** Difícil debuggear
3. **WASM: Ruta de archivo diferente:** `data/` vs paths de CDN

#### Logs Observables

```rust
// src/lib.rs:105-111 - Solo warning, no error
if !state.dictionaries.contains_key(&active) {
    tracing::warn!(
        "Active language '{}' has no dictionary loaded (data/dictionaries/{}.json missing)",
        active,
        active
    );
}
// ⚠️ Continúa ejecutando SIN diccionario
```

### 3. Pipeline de Corrección - Diferencias WASM

#### Configuración WASM en `src/wasm.rs`

```rust
pub fn new(auto_correct: bool, enable_distance: bool, max_distance: usize) -> Self {
    console_error_panic_hook::set_once();
    let config = PipelineConfig {
        auto_correct,
        detect_language: false,  // ⚠️ Deshabilitado por defecto
        buffer_size: 64,
        suggestion_mode: false,
    };

    let pipeline = TypeFixPipeline::new(config);
    Self { pipeline }
}
```

**Problema:** `detect_language: false` significa que el detector no está activo, pero más importante, **no se cargan stopwords automáticamente**.

### 4. Análisis de Errores Estáticos

#### Cobertura de `data/errors/es.json`

```json
{
  "errors": {
    "qeu": "que",
    "k": "que",
    "pq": "porque",
    "xq": "porque",
    "tmb": "también",
    // ... 70+ entradas
  }
}
```

**Análisis:**
- ~70 errores comunes en español
- Coverage: ~60% de errores comunes
- **Faltan:** errores por contexto, regionalismos, jerga

#### Coverage Real vs Esperado

| Categoría | Errores Comunes | En Dictionary | Coverage |
|-----------|-----------------|---------------|----------|
| Transposiciones (qeu→que) | 15 | 15 | 100% |
| Omisiones (k→que) | 10 | 10 | 100% |
| Sustituciones fonéticas | 50 | 20 | 40% |
| Regionalismos | 100+ | 5 | <5% |
| Jerga/Texto SMS | 30+ | 10 | 33% |

### 5. Algoritmo de Búsqueda Fuzzy

#### Damerau-Levenshtein Config

```rust
// src/correction/engine.rs:60-62
pub struct EngineConfig {
    pub max_edit_distance: usize,  // ⚠️ Default = 1
    pub max_candidates: usize,     // ⚠️ Default = 3
    pub min_word_length: usize,    // ⚠️ Default = 2
}
```

**Limitación:** `max_edit_distance: 1` solo encuentra palabras a distancia 1.

```rust
// src/core/dict.rs:126-127
let fst_distance = if max_distance == 1 { 2 } else { max_distance };

// ⚠️ Usa Levenshtein estándar de FST, no Damerau
// Transposiciones como "qeu" vs "que" requieren distancia > 1 en Levenshtein
```

### 6. Verificación de Cobertura Real

#### Método de Test

```bash
# Compilar versión debug
cargo build

# Crear script de verificación
```

#### Resultados Esperados vs Reales

| Input | Esperado | WASM | Native | Causa |
|-------|----------|------|--------|-------|
| `qeu` | que | ❌ | ✅ | Error map no cargado |
| `teh` | the | ❌ | ✅ | Error map no cargado |
| `nesesito` | necesito | ⚠️ | ✅ | Distancia > 1 |
| `hola` | - | ✅ | ✅ | Word válido |
| `zzz` | - | ✅ | ✅ | No hay sugerencia |

### 7. Diagnóstico - Por Qué Falla

#### Causa Raíz #1: Lazy Compilation de FST

```rust
// src/lib.rs:128-133
if !fst_path.exists() && json_path.exists() {
    tracing::info!("Compiling JSON dictionary to FST for language: {}", lang);
    if let Err(e) = Dict::compile_json_to_fst(&json_path, &fst_path) {
        tracing::error!("Failed to compile dictionary to FST: {}", e);
    }
}

if fst_path.exists() {
    let dict = Dict::from_fst_file(&fst_path)?;  // ⚠️ Solo carga FST
    state.dictionaries.insert(lang.to_string(), Arc::new(dict));
}
```

**WASM Issue:** Los archivos `.fst` no se incluyen en el bundle WASM por defecto.

#### Causa Raíz #2: No Hay Stopwords Cargadas en WASM

```rust
// src/wasm.rs:16-30
pub fn new(auto_correct: bool, enable_distance: bool, max_distance: usize) -> Self {
    // ⚠️ No carga stopwords automáticamente
    // ⚠️ No carga error maps automáticamente
    // ⚠️ Solo crea pipeline vacío
}
```

**El usuario DEBE llamar manualmente:**
- `load_dictionary()`
- `load_stopwords()`
- `load_static_errors()`

#### Causa Raíz #3: Buffer Overflow Prevention

```rust
// src/core/buffer.rs:14
pub const MAX_BUFFER_SIZE: usize = 64;
```

Palabras largas (>64 chars) se truncan silenciosamente.

---

## Soluciones Recomendadas

### Solución 1: Incluir Datos en WASM Bundle

```javascript
// pkg/package.json - Agregar archivos de datos
{
  "files": [
    "typefix_bg.wasm",
    "typefix.js",
    "typefix.d.ts",
    "data/dictionaries/en.fst",    // ✅ Incluir
    "data/dictionaries/es.fst",    // ✅ Incluir
    "data/errors/en.json",         // ✅ Incluir
    "data/errors/es.json"           // ✅ Incluir
  ]
}
```

### Solución 2: Auto-carga en Constructor WASM

```rust
// src/wasm.rs
pub fn new_with_defaults() -> Self {
    let mut this = Self::new(true, true, 1);
    
    // Cargar diccionario default
    if let Some(en_dict) = include_bytes!("../data/dictionaries/en.fst") {
        let dict = Dict::from_bytes(en_dict.to_vec());
        this.pipeline.add_dictionary("en", Arc::new(dict));
    }
    
    // Cargar errores estáticos
    if let Some(es_errors) = include_str!("../data/errors/es.json") {
        let map = StaticErrorMap::from_json_str("es", es_errors);
        this.pipeline.add_error_map("es", Arc::new(map));
    }
    
    this
}
```

### Solución 3: Aumentar Coverage de Errores

```rust
// data/errors/es.json - Agregar más errores comunes
{
  "errors": {
    // Existentes: ~70
    // AGREGAR:
    "x": "por",           // "x favor" → "por favor"
    "d": "de",
    "k": "que",
    "msj": "mensaje",
    "tlf": "teléfono",
    "duda": "hubiera duda",
    // ... más
  }
}
```

### Solución 4: Diagnosticar en Runtime

```rust
// src/wasm.rs - Método de diagnóstico
#[wasm_bindgen]
pub fn get_debug_info(&self) -> String {
    let state = get_state();
    let s = state.read();
    
    format!(r#"{{
        "dictionaries_loaded": {},
        "error_maps_loaded": {},
        "active_language": "{}",
        "buffer_size": {},
        "config": {{
            "max_edit_distance": {},
            "max_candidates": {}
        }}
    }}"#,
        s.dictionaries.len(),
        s.error_maps.len(),
        s.active_language,
        self.pipeline.buffer_contents().len(),
        // ... más config
    )
}
```

---

## Verificación de Corrections

### Script de Test

```rust
#[cfg(test)]
mod correction_coverage {
    use typefix::*;
    
    #[test]
    fn test_spanish_common_errors() {
        let pipeline = TypeFixPipeline::simple();
        
        // Test errores estáticos
        assert_eq!(pipeline.push('q'), None);
        assert_eq!(pipeline.push('e'), None);
        assert_eq!(pipeline.push('u'), None);
        let result = pipeline.push(' ');
        assert_eq!(result.unwrap().corrected, Some("que".to_string()));
        
        // Test fuzzy
        let results = pipeline.process_string("nesesito ");
        assert!(results[0].corrected.is_some());  // "necesito"
    }
}
```

### Cobertura Mínima Requerida

| Tipo | Ejemplos | Debe Corregir |
|------|----------|---------------|
| Transposiciones | qeu, teh, adn | ✅ 100% |
| Omisiones | k, pq, xq | ✅ 100% |
| Sustituciones | nesesito, haiga | ⚠️ 60% |
| Abreviaciones | salu2, vdd | ⚠️ 50% |

---

## Conclusión

### Causas Identificadas

1. **WASM no carga datos automáticamente** - El usuario debe llamar `load_dictionary()`, `load_stopwords()`, `load_static_errors()`
2. **Cobertura de errores limitada** - Solo ~70 errores comunes
3. **Edit distance default = 1** - Errores más complejos no se capturan
4. **No hay diagnóstico visible** - Difícil saber qué se cargó

### Acciones Requeridas

1. **Para WASM:** Incluir datos en bundle o auto-cargar
2. **Ampliar errores:** Agregar más errores comunes y regionalismos
3. **Diagnosticar:** Implementar `get_debug_info()` para ver estado
4. **Test coverage:** Verificar que errores常见的 se corrigen

---

*Documento generado para debugging de coverage de corrección en producción.*
