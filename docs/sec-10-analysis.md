# Sección 10 — Análisis de Riesgos y Mitigaciones

**Proyecto:** typefix
**Fecha:** 2026-06-16
**Versión analizada:** 0.1.0 (post Phase 1-4 review)
**Fuentes:** `docs/plan-implementacion.md` (líneas 475-490), `docs/risk-register.md`, código fuente

---

## 1. Resumen Ejecutivo

El análisis de riesgos del `typefix` identifica **25 riesgos** distribuidos en 7 categorías, de los cuales **4 son de severidad crítica**, **6 de severidad alta**, **11 de severidad media** y **3 de severidad baja**. Comparado con el análisis original del plan (8 riesgos), el análisis revisado revela **17 riesgos adicionales** que no fueron anticipados, principalmente relacionados con la incompletitud de los hooks de teclado, calidad de los datos (diccionarios de juguete), riesgos legales/privacidad de HIPAA/GDPR, y patrones de concurrencia sutiles.

**Hallazgo más importante:** El motor no funciona end-to-end en producción. Los tres hooks de teclado (Windows, Linux, macOS) son skeletons con las llamadas reales comentadas. La aprobación de Phase 4 fue prematura.

---

## 2. Lista de Riesgos con Severidad Calculada

| # | ID | Título | Categoría | Probabilidad | Impacto | Severidad (P×I) |
|---|----|--------|-----------|--------------|---------|-----------------|
| 1 | R1 | Hooks no capturan eventos en producción | seguridad | Alta (3) | Crítico (3) | **9 — Crítica** |
| 2 | R5 | log_keystrokes captura contraseñas | seguridad/privacidad | Alta (3) | Crítico (3) | **9 — Crítica** |
| 3 | R10 | Diccionarios de 70-114 palabras, no producción | idiomas/ux | Alta (3) | Crítico (3) | **9 — Crítica** |
| 4 | R18 | Hook receiver() panic en producción | concurrencia/ux | Alta (3) | Crítico (3) | **9 — Crítica** |
| 5 | R2 | WH_KEYBOARD_LL requiere admin | seguridad | Alta (3) | Alto (3) | **6 — Alta** |
| 6 | R3 | CGEventTap requiere Accessibility | seguridad | Alta (3) | Alto (3) | **6 — Alta** |
| 7 | R4 | Linux sin soporte Wayland | seguridad/portab. | Alta (3) | Alto (3) | **6 — Alta** |
| 8 | R6 | find_similar() es O(n) | performance | Alta (3) | Alto (3) | **6 — Alta** |
| 9 | R12 | Falsos positivos en corrección | ux | Alta (3) | Alto (3) | **6 — Alta** |
| 10 | R14 | Sin CJK/RTL/word segmentation | idiomas | Alta (3) | Alto (3) | **6 — Alta** |
| 11 | R19 | Auto-correct no distingue código/texto | ux | Alta (3) | Alto (3) | **6 — Alta** |
| 12 | R7 | Buffer overflow silencioso | ux | Alta (3) | Medio (2) | **4 — Media** |
| 13 | R8 | Damerau ~1MB para strings largas | performance/mem | Media (2) | Alto (3) | **4 — Media** |
| 14 | R11 | pt sin mapa de errores | idiomas | Alta (3) | Medio (2) | **4 — Media** |
| 15 | R13 | Detección de lenguaje puede oscilar | ux/idiomas | Media (2) | Alto (3) | **4 — Media** |
| 16 | R15 | panic = "abort" impide recuperación | memoria/seguridad | Baja (1) | Crítico (3) | **3 — Media** |
| 17 | R16 | Deadlock en listener callbacks | concurrencia | Media (2) | Alto (3) | **4 — Media** |
| 18 | R17 | User errors HashMap sin límite | memoria | Media (2) | Alto (3) | **4 — Media** |
| 19 | R20 | Cross-compilation sin Docker | portabilidad | Alta (3) | Medio (2) | **4 — Media** |
| 20 | R22 | process_string bloquea con paste grande | performance | Media (2) | Medio (2) | **4 — Media** |
| 21 | R23 | UTF-8 corrupto silencioso | seguridad/ux | Media (2) | Medio (2) | **4 — Media** |
| 22 | R25 | Sin rate limiting en API | seguridad/ux | Media (2) | Alto (3) | **4 — Media** |
| 23 | R9 | Vec::remove(0) en word_window | performance | Alta (3) | Bajo (1) | **2 — Baja** |
| 24 | R21 | Detector sin priors reales | idiomas | Alta (3) | Bajo (1) | **2 — Baja** |
| 25 | R24 | macOS OnceLock + Mutex global | concurrencia | Baja (1) | Alto (3) | **2 — Baja** |

> Tabla ordenada por severidad descendente. Mapeo: Alta=3, Media=2, Baja=1, Crítico=3 (igual que Alto para el cálculo).

---

## 3. Top 5 Riesgos Prioritarios

### Top 1 — R1: Hooks no capturan eventos (Severidad 9)
El binario compilado de `typefix` **no captura teclas reales** en ninguna plataforma. El thread de hook solo ejecuta `while !stop_flag { thread::sleep(10ms) }`. Esto significa que el "feature principal" del motor (corrección al teclear) no existe en código. Phase 4 fue aprobada sin verificar end-to-end.

**Acción inmediata:** Descomentar y compilar las APIs reales (`SetWindowsHookExW`, `CGEventTap`, `xcb_grab_keyboard`). Bloquear release v0.1.0 hasta que `cargo run -- repl` capture un keystroke real en al menos una plataforma.

### Top 2 — R18: Hook receiver() panic garantizado (Severidad 9)
Cualquier consumidor que llame `hook.receiver()` en una plataforma real crashea el proceso instantáneamente. Con `panic = "abort"` configurado en `Cargo.toml:80`, el binario muere sin stack trace.

**Acción inmediata:** Cambiar el trait `KeyboardHook` para que `start()` retorne `(JoinHandle, Receiver<HookEvent>)` o exponer el `Sender` ya creado. Eliminar los `panic!` en `windows.rs:178-180`, `linux.rs:218-221`, `macos.rs:281-284`.

### Top 3 — R5: log_keystrokes captura datos HIPAA (Severidad 9)
La flag de logging habilita persistencia de todas las teclas capturadas. Para el target EHR/legal, esto captura PHI (Protected Health Information) — viola HIPAA, GDPR Art. 9, y leyes locales. No hay filtro de campos password.

**Acción inmediata:** Cambiar default a `log_keystrokes: false` en `config.yaml:48`. Añadir filtro de password fields (`EM_GETPASSWORDCHAR`, `isSecureTextField`). Documentar el riesgo legal explícitamente.

### Top 4 — R10: Diccionarios de 70-114 palabras son pruebas, no producción (Severidad 9)
Los archivos JSON en `data/dictionaries/` contienen entre 72 y 114 palabras. Un corrector con 114 palabras tiene cobertura <1% del vocabulario inglés. El target es EHR (50K+ términos técnicos). Un usuario que escriba "diabetes" no obtendrá corrección porque "diabetes" no está en el set de 114.

**Acción inmediata:** Importar diccionarios reales (top 50K de Wikipedia por idioma). Generar JSON reproducible. Documentar el tamaño mínimo aceptable (≥10K).

### Top 5 — R6: find_similar() O(n) — bloquea con 100K palabras (Severidad 6)
`Trie::find_similar()` itera TODAS las palabras del trie para cada corrección. Con un diccionario real, esto es ~10⁹ operaciones de Damerau. Latencia p99 >100ms en producción EHR.

**Acción inmediata:** Cachear `all_words()` con `OnceCell`. Implementar búsqueda pre-filtrada por primera letra. Plan: BK-tree o Symspell para v0.2.0.

---

## 4. Acciones Inmediatas Recomendadas

### Esta semana (Sprint 0 — release blocker)

1. **[R1, R18]** Descomentar y compilar las APIs reales de hooks. Sin esto, no hay producto.
   - `src/hooks/windows.rs:115-134` → activar `SetWindowsHookExW`
   - `src/hooks/macos.rs:185-235` → activar `CGEventTap::new`
   - `src/hooks/linux.rs:139-181` → activar `xcb_grab_keyboard`
   - Owner: Hooks Engineer — **bloqueante**

2. **[R18]** Eliminar los `panic!` en `Hook::receiver()` de las tres plataformas.
   - `windows.rs:178-180`, `linux.rs:218-221`, `macos.rs:281-284`
   - Cambiar a retornar `Receiver` real o exponer en el `HookHandle`.
   - Owner: Hooks Engineer — **bloqueante**

3. **[R5]** Cambiar default `log_keystrokes: false` y añadir filtro de password fields.
   - `config.yaml:48` → `log_keystrokes: false`
   - Implementar detección de campo password por plataforma.
   - Owner: Security + Compliance — **bloqueante legal**

### Este mes (Sprint 1 — quality gate)

4. **[R10]** Importar diccionarios reales de Wikipedia/Wiktionary.
   - Top 50K palabras en, top 30K es, top 30K pt.
   - Script `scripts/build_dictionary.py` reproducible.
   - Owner: Data Engineer — **bloqueante para v1.0**

5. **[R2, R3, R4]** Documentar requisitos de permisos por plataforma.
   - Windows: necesita admin para system mode
   - macOS: necesita Accessibility habilitado
   - Linux: solo X11, no Wayland
   - `README.md` debe advertir antes de la instalación.
   - Owner: DevRel Engineer

6. **[R12]** Implementar `suggestion_mode: true` por defecto en producción.
   - Auto-correct silencioso es muy peligroso para EHR/legal.
   - Owner: UX Engineer

### Próximo trimestre (Sprint 2-4 — quality improvements)

7. **[R6]** Implementar cache para `find_similar()` y pre-filtrado por primera letra.
8. **[R14]** Añadir `unicode-segmentation` al buffer para delimitar palabras.
9. **[R16]** Refactorizar listeners para no invocar bajo lock.
10. **[R17]** Implementar LRU eviction en `user_errors`.

---

## 5. Comparación con los Riesgos Originales del Plan

| # | Riesgo Original (plan-implementacion.md:477-486) | Severidad Original | Estado en Análisis Revisado | Cambio |
|---|--------------------------------------------------|--------------------|----------------------------|--------|
| O1 | Hooks de teclado bloquean input | Media / Alto | **R1: Hooks no capturan eventos** (Crítica) | **Empeoró** — el riesgo real es peor (no capturan nada) |
| O2 | Memory leaks en runtime | Baja / Alto | **R17: User errors sin límite** + **R15: panic=abort** | Empeoró parcialmente |
| O3 | Cambios de idioma espurios | Media / Medio | **R13: Oscilación cerca de threshold** | Igual |
| O4 | Compatibilidad con IME | Media / Alto | **R14: Sin CJK/RTL/segmentation** + **R4: Sin Wayland** | Empeoró |
| O5 | Cross-compilation compleja | Media / Medio | **R20: Sin Docker ni CI** | Empeoró |
| O6 | Crash por panic no capturado | Baja / Crítico | **R15: panic=abort** + **R18: hook receiver panic** | **Empeoró significativamente** |
| O7 | Data race en Trie compartido | Baja / Crítico | **R16: Deadlock en listeners** | Empeoró |
| O8 | Inputs corruptos UTF-8 | Media / Alto | **R23: UTF-8 silencioso** | Igual |

### Riesgos NO anticipados en el plan original (descubiertos en el análisis)

| # | Riesgo Nuevo | Categoría | Severidad |
|---|--------------|-----------|-----------|
| N1 | R5: log_keystrokes captura PHI (HIPAA) | seguridad/privacidad | **9 — Crítica** |
| N2 | R10: Diccionarios de juguete (70-114 palabras) | idiomas/ux | **9 — Crítica** |
| N3 | R19: Auto-correct rompe código/identifiers | ux | 6 — Alta |
| N4 | R11: pt sin errores estáticos (gap de idioma) | idiomas | 4 — Media |
| N5 | R22: process_string bloquea con paste grande | performance | 4 — Media |
| N6 | R25: Sin rate limiting (DoS local) | seguridad | 4 — Media |
| N7 | R21: Detector sin priors (puede fallar en code-switching) | idiomas | 2 — Baja |
| N8 | R24: macOS OnceLock global (anti-pattern) | concurrencia | 2 — Baja |
| N9 | R7: Buffer overflow silencioso (P1-02 abierto) | ux | 4 — Media |
| N10 | R9: Vec::remove(0) ineficiente (P2-01 abierto) | performance | 2 — Baja |

### Resumen de la comparación

- **8 riesgos originales** analizados: 5 empeoraron, 3 igual.
- **10 riesgos nuevos** identificados que no estaban en el plan.
- **0 riesgos originales** cerrados o mitigados completamente desde el plan.
- El plan original **subestimó sistemáticamente** los riesgos de seguridad y privacidad, y **ignoró** los problemas de calidad de datos (diccionarios).

---

## 6. Distribución de Riesgos por Categoría

```
seguridad      ████████████████████  5  (R1, R2, R3, R4, R5, R23*)
performance    ████████████████████  5  (R6, R8, R9, R22, R25*)
memoria        ████████             2  (R17, R15)
concurrencia   ████████████         3  (R16, R18, R24)
ux             ████████████████████  5  (R7, R12, R13, R19, R25*)
idiomas        ████████████████████  5  (R10, R11, R14, R21, R13*)
portabilidad   ████████             2  (R4*, R20)
                * = riesgo en múltiples categorías (contado en cada)
```

### Por severidad

```
Crítica (9)   ████████████████  4
Alta (6)      ████████████████████████  6
Media (4)     █████████████████████████████████████████████ 11
Baja (2)      ████████████  3
```

### Por estado

```
Activo               ██████████████████████████████████████████  22
Aceptado (temporal)  ████████████  3
Mitigado parcial     ████████  2  (R5, R8)
```

---

## 7. Conclusiones

1. **El motor no está listo para producción.** Los hooks no capturan eventos (R1, R18 son bloqueantes absolutos). Phase 4 fue aprobada prematuramente.

2. **Riesgo legal alto.** R5 (log_keystrokes) y la falta de filtros de password fields exponen al equipo a responsabilidad legal bajo HIPAA, GDPR, y leyes locales. Cualquier piloto en EHR debe esperar a resolver R5.

3. **Calidad de datos insuficiente.** R10 (diccionarios de 70-114 palabras) es el riesgo con mayor brecha entre expectativas (README menciona "high-density text input") y realidad. Sin diccionarios reales, la corrección será inútil.

4. **Subestimación sistemática del plan original.** 10 de 25 riesgos no estaban en el análisis original. La revisión debe re-ejecutarse con ojos críticos antes de cualquier release público.

5. **Riesgos técnicos manejables a corto plazo.** R6, R12, R16, R17 son problemas de implementación que pueden resolverse en 1-2 sprints por equipo senior.

6. **Riesgos de plataforma aceptables a largo plazo.** R4 (Wayland), R14 (CJK/RTL) son decisiones de scope — deben documentarse en README y roadmap, no necesariamente resolverse en v0.1.0.

7. **Recomendación final:** No publicar v0.1.0 hasta resolver R1, R5, R10, R18 (los 4 críticos). Los demás riesgos pueden documentarse en el release notes como "known issues" con plan de mitigación.

---

*Análisis generado: 2026-06-16 — válido para v0.1.0 release candidate*
*Próxima revisión recomendada: 2026-07-16 (mensual hasta v1.0)*
