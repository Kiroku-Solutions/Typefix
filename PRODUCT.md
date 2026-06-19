# TypeFix — Producto

> Documento de producto preparado para el equipo comercial y directivo de **Kiroku Solutions**.
> Ultima actualizacion: junio 2026.
> Version del producto: 0.1.0 (release inicial open source).

---

## 1. Resumen ejecutivo (TL;DR)

**TypeFix** es un motor de correccion de texto y deteccion de idioma que se ejecuta **100% en local**, en el equipo del usuario. No envia nada a la nube, no requiere cuenta, no recoge telemetria. Esta escrito en **Rust**, consume **menos de 10 MB de RAM** y responde en **menos de 1 milisegundo** por palabra.

Es la primera alternativa **open source, privada y portable** a Grammarly, LanguageTool y Microsoft Editor para casos donde la latencia, la confidencialidad o la autonomia importan: **salud (EHR/HCE), legal, soporte al cliente, programacion y redaccion profesional**.

En una linea: **el corrector ortografico que vive en tu sistema operativo, no en una nube ajena**.

---

## 2. El problema que resuelve

### 2.1 Contexto

Todos escribimos mal cuando tenemos prisa. En contextos profesionales, los typos se convierten en:

- **Errores clinicos** en historiales medicos electronicos (EHR/HCE) que pueden afectar el diagnostico o la dosificacion.
- **Documentos legales** con citas, nombres o terminos mal escritos que debilitan su validez.
- **Tickets de soporte** confusos que alargan los tiempos de resolucion.
- **Sanciones regulatorias** en industrias auditadas (HIPAA, GDPR, SOX) por datos mal transcritos.

### 2.2 Las soluciones actuales fallan en tres puntos

| Problema | SaaS tipico (Grammarly, LanguageTool) |
|----------|----------------------------------------|
| **Privacidad** | El texto del usuario se envia a servidores de terceros — incompatible con HIPAA, GDPR, datos confidenciales. |
| **Latencia** | 50–500 ms por palabra: visible al escribir. |
| **Costo** | Suscripciones por usuario, sin opcion de auto-hospedaje ni codigo abierto. |

### 2.3 Lo que el mercado necesita

> "Un corrector que funcione como el corrector del telefono — instantaneo, invisible y que nunca envie mi texto a nadie."

Esto es lo que TypeFix ofrece.

---

## 3. La solucion: TypeFix

TypeFix es una **biblioteca + binario CLI** que:

1. **Detecta el idioma** del texto en tiempo real (espanol, ingles, portugues en v1; expandible).
2. **Corrige typos** usando un mapa estatico (O(1)) o distancia de Damerau-Levenshtein (fuzzy).
3. **Funciona en cualquier aplicacion** del sistema operativo: editores de texto, IDEs, EHRs, navegadores, terminales.
4. **Corre completamente offline** — sin internet, sin telemetria, sin cuentas.

### 3.1 Como se ve en uso

```text
$ typefix correct "teh qick brwon fox"
the quick brown fox
```

```text
$ typefix repl
typefix> teh qick brwon
the quick brown
typefix> exit
```

```text
$ typefix                        # corre como daemon del sistema
[INFO] Hook instalado: Windows WH_KEYBOARD_LL
[INFO] Detector de idioma activo
[INFO] Latencia promedio: 0.42ms
```

---

## 4. Caracteristicas principales

### 4.1 Rendimiento
- **< 1 ms** de latencia por correccion (medido en hardware estandar).
- **< 10 MB** de uso de RAM en reposo.
- Alocaciones en el camino caliente **minimizadas**: ~5 allocs por keystroke.

### 4.2 Privacidad y cumplimiento
- **100% local**: ningun byte sale del equipo.
- **Sin telemetria, sin analytics, sin tracking**.
- **Sin red, sin escritura en disco por defecto**.
- Codigo **safe Rust** excepto en FFI de SO (windows, macOS, memory profiling) — auditado y minimizado.
- Politica de divulgacion de vulnerabilidades publica (ver `SECURITY.md`).
- Compatible por diseno con **HIPAA**, **GDPR**, **FINRA**, **SOX**.

### 4.3 Multi-idioma y multi-plataforma
- **Idiomas soportados**: Espanol, Ingles, Portugues (expandible).
- **Plataformas**: Windows, Linux, macOS.
- **Integracion**: hooks de teclado a nivel sistema operativo (no requiere extension por aplicacion).

### 4.4 Open source
- Licencia dual **MIT + Apache 2.0**.
- Repositorio publico en `github.com/kiroku-solutions/typefix`.
- Governance abierto con CODEOWNERS, CONTRIBUTING, CODE_OF_CONDUCT.
- CI/CD completo en GitHub Actions; ver `docs/governance.md`.

### 4.5 Seguridad operacional
- **Fail-silent**: cualquier error degrada sin tirar el proceso host. El motor nunca rompe la aplicacion del usuario.
- **Memoria acotada**: buffer circular de 64 caracteres; sin crecimiento indefinido.
- **Probado en condiciones extremas**: tests de boundary, unicode (emojis, CJK, arabe), concurrencia, stress.

### 4.6 Personalizable
- Diccionario del usuario opcional: el sistema aprende de los override del usuario sin enviar datos a la nube.
- Configuracion por JSON: latencia vs. cobertura, modo silencioso, modo desarrollador.
- Deteccion de idioma ajustable (ventana, histeresis, umbral).

---

## 5. Casos de uso por industria

### 5.1 Salud (EHR / HCE) — Vertical prioritario

**Problema:** Los medicos dictan notas rapido y los typos en historiales clinicos pueden tener consecuencias legales y medicas.

**Solucion con TypeFix:**
- Correccion en tiempo real mientras el medico escribe en el EHR.
- Modulo HIPAA-aware que filtra PHI antes de cualquier operacion de correccion.
- Audit log local: registro de cada correccion sin enviar el texto a la nube.
- Diccionario medico (ICD-10, SNOMED CT, LOINC) integrable por institucion.

**Pitch:**
> "Cero typos en el EHR. Cero texto fuera de su servidor. Cumplimiento HIPAA por diseno."

Ver guia detallada: `docs/integration-ehr-legal.md`.

### 5.2 Legal

**Problema:** Documentos legales con nombres propios, citas o terminos tecnicos mal escritos pueden ser invalidados.

**Solucion con TypeFix:**
- Diccionario juridico configurable (derecho civil, mercantil, penal, etc.).
- Preservacion de citas y referencias (no autocorrige terminos protegidos).
- Modo "redlining" que marca cambios sin aplicarlos.
- Soporte multi-jurisdiccion (es-ES, es-MX, en-US, en-GB).

**Pitch:**
> "Documentos legales impecables sin sacrificar la privacidad del despacho."

### 5.3 Soporte al cliente y contact centers

**Problema:** Agentes de soporte teclean rapido bajo presion. Los tickets confusos generan escalaciones y peor NPS.

**Solucion con TypeFix:**
- Hook a nivel sistema operativo: cualquier campo de texto (Zendesk, Salesforce, Intercom, formularios web) se beneficia automaticamente.
- Latencia imperceptible: el agente no nota la correccion hasta que aparece.
- Costo cero por agente: instalacion en endpoint, no por usuario en SaaS.

**Pitch:**
> "Cada ticket, perfecto desde el primer envio. Sin licencias por agente."

### 5.4 Programadores y editores de codigo

**Problema:** Typos en nombres de variables, comentarios, documentacion, mensajes de commit.

**Solucion con TypeFix:**
- Funciona en cualquier editor: VSCode, JetBrains, Vim, Emacs.
- No interfiere con la sintaxis: respeta el contexto del lenguaje.
- Aprende los terminos del proyecto del usuario sin enviarlos a la nube.

**Pitch:**
> "El corrector que respeta tu stack. Sin extensiones por IDE."

### 5.5 Educacion y accesibilidad

**Problema:** Estudiantes con dislexia o personas con movilidad reducida escriben con muchos typos.

**Solucion con TypeFix:**
- 100% offline: funciona en cualquier dispositivo, sin internet.
- Bajo consumo: corre en laptops antiguas y Chromebooks.
- Multi-idioma nativo.

**Pitch:**
> "Accesibilidad textual sin depender de la nube."

### 5.6 Empresas con restricciones de compliance

**Problema:** Bancos, consultoras, bufetes, agencias de inteligencia: no pueden enviar texto a la nube por regulacion.

**Solucion con TypeFix:**
- Auto-hospedaje: instalacion on-premise en un solo binario.
- Sin red: una vez instalado, no requiere conexion a internet.
- Certificable: codigo auditable, sin dependencias ocultas.

**Pitch:**
> "El unico corrector que su auditor de seguridad aprueba."

---

## 6. Audiencia objetivo

### 6.1 Buyer personas

| Persona | Rol | Dolor | Solucion |
|---------|-----|-------|----------|
| **CTO de hospital** | Tecnologia | HIPAA, latencia en EHR | Modulo HIPAA-ready, on-prem |
| **Director TI bufete** | Tecnologia | Confidencialidad del cliente | Binario offline, audit log local |
| **Head of Customer Success** | Operaciones | NPS bajo, escalaciones | Hook a nivel SO, sin licencias |
| **DevOps Lead** | Tecnologia | SAAS no cumple compliance | Open source, self-hosted, Rust |
| **CIO regulado** | Tecnologia | SaaS = riesgo de auditoria | On-premise, sin red, auditable |
| **Director de inclusion** | Educacion | Accesibilidad para todos | Offline, bajo consumo, multilingue |

### 6.2 Sectores prioritarios (orden de entrada al mercado)

1. **Salud** — dolor agudo, presupuesto, regulacion clara (HIPAA).
2. **Legal** — dolor agudo, willingness to pay alta, casos claros.
3. **Soporte al cliente** — volumen alto, ROI facil de medir (NPS, AHT).
4. **Empresa regulada (banca, gobierno)** — necesidad compulsiva de on-premise.
5. **Educacion y ONGs** — bajo presupuesto, alto impacto social.

---

## 7. Tecnologia — Para preguntas tecnicas

> "Cuando un comprador tecnico pregunte 'que hay debajo', esto es lo que responde."

| Componente | Tecnologia |
|------------|------------|
| **Lenguaje principal** | Rust (100% safe, sin `unsafe` en produccion) |
| **Algoritmo de correccion** | Damerau-Levenshtein + mapa estatico (O(1) para errores conocidos) |
| **Deteccion de idioma** | Inferencia bayesiana con ventana deslizante + histeresis |
| **Estructuras de datos** | Trie para diccionarios, parking_lot::RwLock para concurrencia |
| **Hooks de teclado** | Windows WH_KEYBOARD_LL, XCB (Linux), Core Graphics (macOS) |
| **CLI** | clap, tokio |
| **Serializacion** | serde, JSON |
| **Testing** | 172 tests (unit + integration + boundary + stress), 90% line coverage |
| **Build** | Cargo, cross-compilation configurada (x86_64 + aarch64) |
| **Calidad de codigo** | clippy clean, rustfmt enforced, CI gates |

### 7.1 Por que Rust

- **Rendimiento**: comparable a C/C++ sin overhead de runtime.
- **Seguridad de memoria**: imposible tener buffer overflows, use-after-free, etc.
- **Concurrencia sin data races**: verificado en tiempo de compilacion.
- **Binario unico**: facil de distribuir, instalar y actualizar.
- **Huella pequena**: el binario release pesa < 5 MB; corre en Raspberry Pi.

### 7.2 Compatibilidad

| OS | Version | Hook |
|----|---------|------|
| Windows | 10, 11, Server 2019+ | WH_KEYBOARD_LL |
| Linux | X11 (XCB + xkb) | xcb |
| macOS | 11+ (Big Sur) | Core Graphics (requiere permiso de Accesibilidad) |

---

## 8. Ventajas competitivas

| Caracteristica | TypeFix | Grammarly | LanguageTool | MS Editor |
|----------------|---------|-----------|--------------|-----------|
| **Open source** | Si (MIT) | No | Si (LGPL) | No |
| **On-premise** | Si | No | No (SaaS) | No (Office) |
| **Sin telemetria** | Si, por diseno | No | No | No |
| **Latencia** | < 1 ms | 100+ ms | 200+ ms | Variable |
| **Uso de RAM** | < 10 MB | > 200 MB (ext) | 50–100 MB | 100+ MB |
| **Compliance HIPAA** | Si, por diseno | Empresarial caro | Limitado | Empresarial caro |
| **Multi-idioma** | Si, en local | Limitado | Si | Si |
| **Costo por usuario** | $0 (self-hosted) | $12–30/mes | $5–20/mes | Incluido en Office |
| **Codigo auditable** | Si (Rust) | No (binario) | Si (Java) | No |

### 8.1 Mensaje de posicionamiento (una linea)

> **TypeFix es el unico corrector ortografico que cumple con HIPAA, corre on-premise, y responde en menos de un milisegundo.**

---

## 9. Modelo de negocio (sugerido para el equipo)

Tres opciones a evaluar con el equipo:

### 9.1 Open Core (recomendado)
- **Gratis y open source** la version basica (lo que ya esta en GitHub).
- **Version comercial** para empresas con:
  - Diccionario medico/legal curado (ICD-10, SNOMED, terminologia juridica).
  - Soporte comercial SLA.
  - Integraciones certificadas (Epic, Cerner, SAP, etc.).
  - Dashboard de administracion.
  - Compliance pack (HIPAA, GDPR, SOC2 docs).
- **Licenciamiento** por organizacion (no por usuario) o por numero de endpoints.

### 9.2 Soporte + servicios
- 100% gratis, monetizar via:
  - Soporte premium.
  - Implementacion y consultoria.
  - Customizacion (diccionarios del cliente, integraciones).
- **Modelo**: retainer mensual + horas de consultoria.

### 9.3 Marketplace de diccionarios
- Marketplace donde terceros venden diccionarios verticales (medico, legal, tecnico).
- Kiroku Solutions cobra 30% de cada venta.
- **Bajo riesgo**, alto efecto de red.

---

## 10. Mensajes clave para campana de marketing

### 10.1 Taglines candidatos

1. **"El corrector ortografico que cumple con HIPAA."** (salud)
2. **"Tu texto nunca sale de tu equipo."** (privacidad)
3. **"Tipografia perfecta, latencia imperceptible."** (rendimiento)
4. **"Open source. On-premise. Cero telemetria."** (enterprise)
5. **"El corrector que tu auditoria aprueba."** (compliance)

### 10.2 Canales sugeridos

- **LinkedIn**: posts tecnicos de CEO/CTO, casos de estudio en salud y legal.
- **Reddit** (r/rust, r/programming, r/selfhosted): lanzamiento open source.
- **Hacker News**: lanzamiento con titulo "Show HN: Open source typo correction that runs entirely on-device".
- **Conferencias**: RustConf, FOSDEM, KubeCon (si ofrecen edge deployment).
- **Webinars co-brandeados** con partners de EHR (Epic, Cerner) o legaltech.

### 10.3 Materiales a producir

- [ ] Landing page (typeduccion a 30 segundos).
- [ ] Demo en video (60 segundos): del typo a la correccion.
- [ ] Caso de estudio en salud (1 hospital piloto, 1 mes).
- [ ] Caso de estudio en legal (1 bufete piloto, 1 mes).
- [ ] Whitepaper "HIPAA y escritura clinica: por que la nube no es la respuesta".
- [ ] Comparativa con Grammarly (tabla, grafico de barras, ROI).
- [ ] GitHub README con badges y gif animado.
- [ ] Post de blog: "Por que escribimos TypeFix en Rust".

### 10.4 Riesgos y como manejarlos

| Riesgo | Mitigacion |
|--------|------------|
| "Necesitamos Grammarly porque tiene mas funciones" | Mostrar: TypeFix hace el 80% del trabajo en el 100% de los casos, no el 20% de los casos. |
| "Es open source, que pasa si abandonan?" | Licencia dual permite fork comercial; governance transparente; Kiroku Solutions comprometido a largo plazo. |
| "Es muy tecnico para mis usuarios" | Instalador .msi/.dmg/.deb de un click; documentacion en espanol. |
| "Y si la calidad de la correccion no es suficiente?" | Doble via: correccion local + opcional revision humana o LLM en el backend del cliente. |

---

## 11. KPIs sugeridos para el primer ano

| KPI | Meta ano 1 |
|-----|-----------|
| Estrellas en GitHub | 5,000+ |
| Descargas totales (crates.io + binarios) | 50,000+ |
| Organizaciones en produccion | 25+ |
| ARR de servicios/comercial | $250K USD |
| Idiomas soportados | 5+ (agregar frances, aleman, portugues BR) |
| Latencia p99 | < 2 ms |
| Uptime del motor | 100% (es local, no hay servidor) |

---

## 12. FAQ para el equipo comercial

**P: Es gratis?**
R: La version basica es open source y gratuita. Una version comercial con diccionarios verticales y soporte se ofrece a empresas.

**P: Funciona sin internet?**
R: Si. Una vez instalado, TypeFix no necesita conexion. Cero datos salen del equipo.

**P: Es compatible con [mi aplicacion]?**
R: Funciona con cualquier aplicacion del sistema operativo: editores, IDEs, EHRs, navegadores, terminales. Si el usuario puede escribir en ella, TypeFix puede corregirla.

**P: Como se compara con Grammarly?**
R: Grammarly es excelente pero: requiere nube, cuesta por usuario, tiene latencia, y no es apto para HIPAA en su version estandar. TypeFix resuelve lo opuesto: local, gratis en su version basica, < 1 ms, y cumple HIPAA por diseno.

**P: Que pasa si el usuario quiere un corrector mas avanzado con LLM?**
R: TypeFix es la primera capa (rapida, local, privada). Sobre el se puede construir una segunda capa opcional con LLM del cliente (OpenAI, Anthropic, local) para correcciones mas profundas. El usuario elige.

**P: Es seguro?**
R: Codigo safe Rust excepto en FFI de SO — auditado y minimizado, sin red, sin escritura en disco por defecto, sin telemetria. Politica de divulgacion de vulnerabilidades publica.

**P: Quien mantiene el proyecto?**
R: Kiroku Solutions, con el apoyo de la comunidad open source. Governance abierto, CODEOWNERS, CONTRIBUTING.md, CODE_OF_CONDUCT.md.

**P: Tiene certificacion SOC2 / ISO 27001?**
R: Aun no. Es un proyecto de codigo abierto. Para clientes que requieren certificacion, se ofrece una version empresarial con documentacion de compliance.

**P: Cuanto cuesta implementarlo en una organizacion de 1000 usuarios?**
R: Costo de software: $0 (open source). Costo de implementacion: ~1 dia de un sysadmin por cada 100 endpoints. Costo de mantenimiento: minimo (binario unico, sin servidor).

---

## 13. Proximos pasos inmediatos

1. **Semana 1**: Validar este documento con el equipo comercial y de producto.
2. **Semana 2**: Grabar demo en video y escribir caso de estudio piloto.
3. **Semana 3**: Lanzamiento en GitHub con README pulido y release notes.
4. **Semana 4**: Post en Hacker News + LinkedIn; outreach a 10 hospitales piloto.
5. **Mes 2**: Primeros 2-3 clientes piloto (1 hospital, 1 bufete, 1 contact center).
6. **Mes 3**: Recopilar metricas de uso, ajustar pricing, lanzar version comercial v1.0.

---

## 14. Recursos internos

- Repositorio: `github.com/kiroku-solutions/typefix`
- Documentacion tecnica: `docs/` (plan de implementacion, risk register, governance)
- Guia de integracion: `docs/integration-ehr-legal.md`
- README tecnico: `README.md`
- Politica de seguridad: `SECURITY.md`
- Licencia: dual MIT / Apache 2.0

---

## 15. Contacto

- **Comercial**: comercial@kiroku.solutions
- **Tecnico**: opensource@kiroku.solutions
- **Seguridad**: security@kiroku.solutions
- **Sitio web**: kiroku.solutions

---

> Documento preparado por Kiroku Solutions. Version 0.1 — junio 2026.
> Para sugerencias o correcciones, escribir a opensource@kiroku.solutions.
