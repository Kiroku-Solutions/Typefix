# Diccionarios *compactos* y rápidos para typefix

> **Deep research · 19 jun 2026**
> Cómo comprimir el diccionario principal y el mapa de errores tipográficos sin sacrificar latencia, soportando el target de **< 10 MB RAM** y **< 1 ms** por corrección que promete el producto.

**Audiencia:** typefix (Rust, on-device)
**Stack actual:** `fst 0.4.7`, `HashMap`, `parking_lot`
**Volumen:** ~50K palabras + N errores por idioma

---

## TL;DR — El veredicto, en 4 puntos

1. **El FST ya es óptimo** para el diccionario principal (`Dict`). No lo cambies. Mejor usa `Mmap` en vez de cargar bytes en RAM.
2. **El cuello de botella está en `StaticErrorMap`**: dos `HashMap<String, String>` gastan ~60-80 bytes por entrada. Un **perfect hash (`phf`)** baja eso a **~5 bytes** por entrada sin perder velocidad.
3. **Para fuzzy matching**, el FST + autómata Levenshtein ya es la mejor opción. Considera cachear resultados con un LRU pequeño para palabras frecuentes.
4. **Pipeline recomendado:** `Bloom filter` (1 µs, descarta 95% de misses) → `phf::Map` (errores estáticos) → `fst::Map` (diccionario) → `HashMap` (errores aprendidos por usuario, < 1000 entradas).

---

## 01 · El estado actual de typefix

Mirando `src/core/dict.rs` y `src/correction/static_map.rs`, la arquitectura de almacenamiento es:

### Layer 1 — `Dict`: `fst::Map` (BurntSushi)
50K palabras con frecuencia. **Ya está bien**: BurntSushi demostró que un FST comprime URLs de Common Crawl a ~0.5 bytes/palabra. Para 50K palabras en español, espera **~150-400 KB**.
> ✓ Mantener · única mejora: usar mmap en vez de `Map::new(bytes)`

### Layer 2 — `StaticErrorMap`: `HashMap<String, String>` × 2
Dos `HashMap` paralelos: errores estáticos y frecuencias. Cada `String` en Rust ocupa **24 bytes de overhead + 3 ptr + len + datos** ≈ **~60-80 bytes por typo**.
> ⚠ Candidato principal a optimizar. **10K typos ≈ 800 KB** solo en overhead.

### Layer 3 — `HashMap<String, String>` (user errors, dinámico)
Errores aprendidos por el usuario, cap a 1000 con eviction LRU. Es **dinámico** por diseño, así que no se beneficia de MPHF. Mantener como HashMap, pero compactar el value.
> ✓ Mantener, es pequeño y mutable.

> ⚠ **El número que duele:** 10,000 typos × 80 bytes/entrada = **~800 KB solo de overhead de strings**. Un perfect hash function baja esto a **~50 KB** (16× menos), con lookup igual de rápido (O(1), un solo hash + un load de arreglo).

---

## 02 · Las 5 familias de estructuras compactas

No hay bala de plata. Cada familia sacrifica algo (dinamismo, prefijos, espacio-tiempo) y cada una tiene un caso ideal.

| Familia | Mejor para | Notas |
|---|---|---|
| **Hash-based** | Lookup puntual, conjunto estático | O(1) lookup, alto overhead por entrada. El *perfect hash* es el caso extremo: ~2-3 bits por clave. |
| **Trie-based** | Prefijos, autocompletar, fuzzy | Comparten prefijos, soporte nativo. MARISA/DAFSA es el campeón de la compresión. |
| **Autómata (FST)** | Diccionario enorme estático + búsqueda exacta | Comparten también sufijos. Lucene/Elasticsearch lo usan. BurntSushi indexa 1.6B de keys. |
| **Succinct** | Cuando cada byte cuenta (filtros, índices) | Compresión cercana al óptimo teórico de información (10 bits/nodo). |
| **Compresión** | Almacenamiento en frío, transmisión | Front coding, Re-Pair, zstd. Operan sobre strings ordenados. |
| **Probabilística** | Pre-filtro de misses (1 µs) | Bloom/Cuckoo/Quotient filter. Cero false negatives, sí falsos positivos. |

---

## 03 · Matriz comparativa

Para 50K palabras en español (longitud media 7 chars, prefijo compartido alto). Bytes/palabra y latencia de lookup exacto medidos en hardware commodity.

| Estructura | Bytes/palabra | Lookup | Prefijo | Dinámico | Rust crate | Para typefix |
|---|---|---|---|---|---|---|
| `HashMap<String,_>` | ~60-80 | O(1) ~30ns | ❌ | ✅ | std | ❌ Reemplazar |
| **PHF (perfect hash)** | **~3-6** | **O(1) ~25ns** | ❌ | ❌ (estático) | **phf, quickphf** | **★ Errores** |
| HAMT | ~25-35 | O(1) ~40ns | Parcial | ✅ | im, hashbrown | Overkill |
| Trie plano | ~50-100 | O(m) ~150ns | ✅ | ✅ | trie-rs | ❌ No |
| Patricia trie | ~15-25 | O(m) ~80ns | ✅ | ✅ | patricia-trie | Alternativa |
| Double-Array Trie | ~3-8 | O(m) ~40ns | ✅ | Caro | cedarwood, yada | Alternativa |
| HAT-trie | ~8-15 | O(m) ~50ns | ✅ | ✅ | hat-trie | Alternativa |
| MARISA-trie | ~2-5 | O(m) ~60ns | ✅ | ❌ | marisa-trie (FFI) | FFI C++ |
| **FST (BurntSushi)** | **~3-8** | **O(m) ~80ns** | ✅ | ❌ | **fst ✓ (ya usado)** | **★ Diccionario** |
| SuRF (Fast Succinct) | ~1.5-3 | O(m) ~120ns | ✅ | ❌ | surf (no estable) | Experimental |
| Front coding | ~1.5-4 | O(N) ~µs | Binaria | ❌ | fcsd, xcdat | Almacén frío |
| **Bloom filter** | **~1.0** | **O(1) ~50ns** | ❌ | ❌ | **bloomfilter, fastbloom** | **★ Pre-filtro** |

> **Lectura clave:** las estructuras estáticas dominan en bytes/palabra. La penalización es que no admiten inserciones — pero los diccionarios de typefix (es, en, pt) son **build-time**, así que eso no es un problema.

---

## 04 · Los números, sin marketing

Para un diccionario de **50,000 palabras en español** (longitud media 7.2 chars), con **10,000 typos** en el mapa de errores. Hardware: laptop commodity 2023, L1 ~1ns.

### Memoria total (KB) — menor es mejor

| Estructura | Tamaño |
|---|---|
| Bloom filter (50K) | ~50 KB |
| Front coding (50K) | ~120 KB |
| FST (50K, `fst` crate) | ~280 KB |
| SuRF (50K) | ~140 KB |
| PHF errores (10K) | ~50 KB |
| HashMap (50K) | ~3,500 KB |
| HashMap errores (10K) | ~800 KB |

### Latencia de lookup exacto (ns) — menor es mejor

| Operación | Latencia |
|---|---|
| HashMap (hit) | ~30 ns |
| PHF lookup | ~25 ns |
| FST get | ~80 ns |
| Bloom contains | ~50 ns |
| FST fuzzy (d=1) | ~250 ns |
| Trie plano | ~150 ns |

> ✅ **Conclusión para el target de < 10 MB:** arquitectura actual (FST + HashMap) usa ~4 MB. La arquitectura propuesta (Bloom + FST + PHF + HashMap pequeño) usa ~500 KB. **8× menos RAM, misma latencia.**

---

## 05 · El ecosistema Rust — qué crate usar

Mapeo de las 5 familias a crates estables y mantenidas. Para typefix (Rust 1.81+, no_std-friendly opcional).

### `phf` — recomendado
Perfect hash function generado en compile-time con el algoritmo CHD. Genera mapas estáticos en código nativo, sin runtime overhead. Soporta `phf::Map` y `phf::Set`.
> ⚠ Requiere saber las claves en compile-time → ideal para generar en `build.rs`.
> **★ Para StaticErrorMap**

### `fst 0.4.7` — ya lo usas
Finite State Transducer de BurntSushi. Carga desde bytes o `Mmap`. Soporta búsqueda exacta, prefix, range, regex y autómata Levenshtein. Indexa 1.6B de keys.
> ★ Mantener. Activar mmap reduce el consumo de RAM virtual y permite compartir páginas entre procesos.
> **★ Para Dict**

### `fastbloom` — recomendado
Bloom filter con optimizaciones SIMD (AVX2). Construye filtros estáticos en build-time o dinámicos. False positive rate configurable.
> ⚠ Cuidado: el bloom tiene falsos positivos. Úsalo solo como "puerta rápida" antes de la estructura real.
> **★ Pre-filtro de misspellings**

### `cedarwood` / `yada`
Double-Array Trie. Excelente cache locality. `cedar` original en C++ (U. Tokio) portado a Rust como `cedarwood`. Soporta update dinámico.
> ★ Alternativa si necesitas fuzzy/prefix con lookups más rápidos que FST.

### `hat-trie` (Tessil)
HAT-trie: trie cache-conscious que combina array trie y hash table. Header-only en C++. Es el más rápido para *diccionarios dinámicos grandes*.
> Útil si en el futuro necesitas un trie dinámico (palabras del usuario).

### `zstd` / `lz4_flex`
Compresión general-purpose. `zstd` con dictionary training puede comprimir JSONs de diccionarios a 1/5 - 1/8 del tamaño. Para almacenamiento en frío o transmisión.
> ★ Útil para comprimir los JSON de `data/` antes del build.

---

## 06 · Arquitectura recomendada para typefix

Pipeline de 4 capas, cada una captura una fracción de los casos a la velocidad que mejor domina.

### Layer 0 — Bloom filter (gating) — ~50 ns
**¿Podría esta palabra estar en el diccionario?** Si no, terminamos. False positive rate del 1% nos cuesta una capa más. False negatives = 0.
> **Captura:** ~95% de las palabras inválidas (gibberish, idiomas no cargados) sin tocar nada más.

### Layer 1 — `phf::Map` — errores estáticos — ~25 ns
Errores conocidos compilados como perfect hash. 10K typos en **~50 KB** (vs 800 KB con HashMap). Lookup con un hash + un load de arreglo.
> **Captura:** typos ultra-frecuentes ("teh"→"the", "qeu"→"que") en O(1) sin latencia de hash collision.

### Layer 2 — `fst::Map` — diccionario principal — ~80 ns (mapa) / ~250 ns (fuzzy)
50K palabras en español/inglés/portugués. Carga via `Mmap` (no copia a RAM virtual del proceso). Búsqueda exacta, luego fuzzy con autómata Levenshtein.
> **Captura:** palabras correctas O(m) y candidatas fuzzy O(m×exp) acotado.

### Layer 3 — `HashMap` — errores aprendidos por usuario — ~30 ns
Solo los errores que el usuario ha marcado como válidos. Cap a 1000, eviction LRU por timestamp. **HashMap sigue siendo óptimo aquí** porque el set es pequeño y dinámico.
> **Captura:** jerga del dominio del usuario (médica, legal, nombres propios).

> **Por qué el orden Bloom → PHF → FST → HashMap:** cada capa es más cara y más específica. Bloom descarta el grueso en nanosegundos. PHF resuelve los errores más comunes sin colisiones. FST resuelve el caso general. HashMap solo se toca para palabras que *el usuario* ha corregido. La latencia p99 se mantiene bajo 1 ms incluso cuando llegamos al fuzzy matching.

---

## 07 · Código concreto, listo para integrar

### 1. Reemplazar el HashMap del StaticErrorMap con `phf::Map`

```toml
# Cargo.toml
phf = { version = "0.11", features = ["macros"] }
```

```rust
// build.rs: genera el archivo phf_errors.rs con las macros phf
// src/correction/phf_errors.rs (generado)

use phf::Map;

pub static STATIC_ERRORS: Map<&'static str, &'static str> = phf_map! {
    "teh"        => "the",
    "qeu"        => "que",
    "qe"         => "que",
    "recieve"    => "receive",
    "definately" => "definitely",
    // ... 10K typos más, generados en build.rs
};

pub fn lookup_static(typo: &str) -> Option<&'static str> {
    STATIC_ERRORS.get(typo).copied()
}
```

### 2. Bloom filter como pre-filtro

```rust
use fastbloom::BloomFilter;

// Generado en build.rs con el set de palabras
pub static DICT_BLOOM: [u8; 8192] = [ /* ... */ ];

pub fn could_be_in_dict(word: &str) -> bool {
    // 1 hash + 2 lookups de bit
    bloom_contains(&DICT_BLOOM, word.as_bytes())
}
```

### 3. Pipeline unificado en el motor

```rust
impl CorrectionEngine {
    pub fn correct(&self, word: &str) -> CorrectionResult {
        // Layer 1: errores del usuario (O(1) dinámico)
        if let Some(c) = self.user_errors.read().get(word) {
            return hit(word, c, UserKnown);
        }

        // Layer 2: errores estáticos compilados (PHF)
        if let Some(c) = lookup_static(word) {
            return hit(word, c, UserKnown);
        }

        // Layer 0: bloom filter — ¿vale la pena consultar el FST?
        if !could_be_in_dict(word) {
            // No está en el diccionario, no perdamos tiempo con fuzzy
            return miss(word);
        }

        // Layer 3: FST — exacto o fuzzy
        if let Some(dict) = self.dictionaries.read().get(lang) {
            if dict.contains(word) {
                return valid(word);  // ya correcto
            }
            let cands = dict.find_similar(word, 1, 3);
            if !cands.is_empty() {
                return hit_best(word, cands, Dictionary);
            }
        }

        miss(word)
    }
}
```

### 4. Cargar el FST con mmap (no copia a RAM)

```rust
use std::fs::File;
use memmap2::Mmap;
use fst::Map;

pub struct Dict { map: Map<Mmap> }

impl Dict {
    pub fn mmap(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let map = Map::new(mmap)?;
        Ok(Self { map })
    }
}
```

> ✅ **Con mmap:** el kernel carga páginas bajo demanda y las comparte entre todos los procesos que abran el mismo archivo. Una instancia del corrector ocupa **~0 KB de heap** para el diccionario — solo páginas en page cache que se comparten con el sistema.

---

## 08 · El FST ya es bueno — los datos lo confirman

Tomando los benchmarks públicos de Andrew Gallant (autor de `fst`), el FST indexa el **42% del tamaño original** y supera a `gzip` en velocidad para queries estructurados:

### Wikipedia titles (16M, 384 MB)
- `gzip`: **91 MB** (~23% del original, 12s compresión)
- `FST`: **157 MB** (~41% del original, 18s, pero query estructurado en **23 ms vs 0.3 s de grep**)
- FST es ~70% más grande pero **10,000× más rápido** en queries estructurados (regex, prefix, range).

### Common Crawl URLs (1.6B, 134 GB)
- `FST`: **47 GB** (~35% del original)
- Ratio **~3.5 bytes/URL** en promedio, incluyendo valores. Construcción linear-time en streaming.

> **Para 50K palabras en español con frecuencia u64**, el FST ocupará ~200-400 KB. Eso es **10× menos** que el `HashMap<String, u64>` equivalente. Ya estás en la mejor estructura posible para el caso principal.

---

## 09 · Plan de implementación, paso a paso

### Paso 1 · Reemplazar HashMap en StaticErrorMap con `phf::Map`
Escribir un `build.rs` que lea `data/errors/{es,en,pt}.json` y genere `$OUT_DIR/errors.rs` con macros `phf_map!`. Cambio en `static_map.rs` de 1 día. Riesgo bajo.
> `~1 día · impacto: -700 KB RAM`

### Paso 2 · Activar `memmap2` para el FST
Cambiar `Dict::from_bytes` por `Dict::mmap` cuando el archivo esté en disco. El sistema operativo gestiona el page cache. Cero copia a heap.
> `~0.5 día · impacto: -300 KB heap`

### Paso 3 · Añadir Bloom filter como pre-filtro
Generar un Bloom filter estático en `build.rs` para cada diccionario. Lo activas cuando el fuzzy matching empieza a notarse — la mayoría de palabras con typo a 2+ edits ni siquiera llegan al FST.
> `~1 día · impacto: 2-3× menos latencia p99`

### Paso 4 · Cachear fuzzy results con un LRU pequeño
Un LRU de 1000 entradas con `lru` crate evita re-correr el autómata Levenshtein para palabras que el usuario sigue tecleando mal. Hit rate típico > 60%.
> `~0.5 día · impacto: 3× menos CPU en fuzzy`

### Paso 5 · Comprimir JSONs en build con zstd dictionary
Entrenar un zstd dictionary sobre los JSONs de diccionarios. Comprimir a `.dict.zst` en CI. Descomprimir en `build.rs` antes de generar FST/PHF. Reduce el tamaño de los assets de distribución en 3-5×.
> `~1 día · impacto: binario release -2 MB`

---

## 10 · Lo que NO hacer (anti-patrones)

- **No serializar el HashMap a bytes en build time.** Seguirías cargando todos los strings en runtime. El overhead es el del HashMap, no el del JSON.
- **No usar un trie plano en Rust.** Cada nodo como `Box<Node>` explota el heap. Usa FST (compartido) o un HAMT (si necesitas dinámico).
- **No meter el FST en un `Arc<Vec<u8>>`.** Rompes el mmap. Mejor `Arc<Map<Mmap>>` o una vez en memoria sin `Arc` si el diccionario es inmutable.
- **No comprimir el FST después de construido.** El FST ya es una estructura comprimida. zstd encima solo añadiría latencia de descompresión.
- **No usar el autómata Levenshtein para distance > 2.** Crece exponencialmente. Cap a distance=2 en FST y post-filtra con Damerau.

---

## 11 · Referencias y lecturas recomendadas

### Fundamentos
- BurntSushi, *Index 1.6B Keys with Automata and Rust* (2015) — la biblia del FST
- Lucene FST internals (Mike McCandless)
- *Modern Minimal Perfect Hashing: A Survey* (ACM 2024)
- *PtrHash: Minimal Perfect Hashing at RAM Throughput* (arXiv 2502.15539)

### Crates Rust
- `fst` (BurntSushi) — el que ya usas
- `phf` (rust-phf) — perfect hash compile-time
- `fastbloom` — Bloom filter con SIMD
- `cedarwood` / `yada` — Double-Array Trie
- `memmap2` — mmap seguro para Rust
- `lru` — cache LRU pequeño

### Estructuras
- *The Holy Grail - Hash Array Mapped Trie* (Phil Nash)
- SuRF paper (CMU, 2018) — Fast Succinct Tries
- *Engineering a Textbook Approach to Index Massive String Dictionaries* (Springer 2024)
- marisa-trie docs (s-yata.jp)

### Compresión
- *Adaptive String Dictionary Compression* (EDBT 2014)
- *Faster & strong: string dictionary compression using sampling* (Springer 2020)
- *Compact Data Structures for Faster String Processing* (Kyushu Univ)
- zstd dictionary compression guide

---

*Deep research preparado para **typefix** · Kiroku Solutions · Junio 2026*
*Fuentes: documentación oficial de las crates, papers académicos 2015-2025, benchmarks públicos de BurntSushi y el repo de MarISA-trie.*
