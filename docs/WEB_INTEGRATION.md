# TypeFix WebAssembly (WASM) Integration Guide

¡Bienvenido a la guía de integración web de TypeFix! Con esta guía podrás llevar la potencia del motor de corrección ultrarrápido y de auto-switch inteligente directamente al navegador de tus usuarios, sin necesidad de un backend o llamadas a API.

TypeFix se compila a WebAssembly (WASM), permitiendo a tu aplicación JavaScript (React, Vue, Vanilla) inicializar un pipeline de procesamiento de texto con una latencia de 0 milisegundos.

---

## 1. Instalación y Compilación

Primero, debemos empaquetar TypeFix como un paquete NPM. Para ello utilizamos `wasm-pack`:

```bash
# Instala wasm-pack si no lo tienes
cargo install wasm-pack

# Compila el paquete para la web
wasm-pack build --target web --out-name typefix-wasm --out-dir pkg
```

Esto generará una carpeta `/pkg` con los archivos `.js` y `.wasm` listos para usar en tu proyecto web.

---

## 2. Inyección de Diccionarios y Stopwords

En el entorno web, no existe el archivo local `config.json`. En su lugar, el Front-End es el responsable de descargar los recursos (Diccionarios FST y listas JSON) mediante llamadas `fetch()`.

### Estructura de archivos estáticos
Sube tus archivos de idioma al directorio público de tu servidor o CDN:
```text
/public
  ├── dictionaries/
  │   ├── en.fst
  │   ├── es.fst
  ├── stopwords/
  │   ├── en.json
  │   ├── es.json
```

> [!TIP]
> **Rendimiento**
> Los archivos `.fst` son binarios extremadamente compactos. Un diccionario de 600,000 palabras en español suele pesar solo 1.5MB. ¡Cargarán instantáneamente en la web!

---

## 3. Ejemplo Práctico de Inicialización (JavaScript)

El siguiente código demuestra cómo importar el paquete, descargar los idiomas y escuchar un área de texto:

```javascript
import init, { TypeFixWeb } from './pkg/typefix_wasm.js';

async function startTypeFix() {
    // 1. Inicializar el motor WASM
    await init();

    // 2. Crear la instancia. (auto_correct=true, detect_language=true, buffer_size=64)
    const typefix = new TypeFixWeb(true, true, 64);

    // 3. Descargar e inyectar el diccionario en Español
    const esDictResponse = await fetch('/dictionaries/es.fst');
    const esDictBuffer = await esDictResponse.arrayBuffer();
    typefix.loadDictionary('es', new Uint8Array(esDictBuffer));

    // 4. Descargar e inyectar Stopwords (para el Auto-Switch)
    const esStopwordsResponse = await fetch('/stopwords/es.json');
    const esStopwordsJson = await esStopwordsResponse.text();
    typefix.loadStopwords('es', esStopwordsJson);
    
    // Repite los pasos 3 y 4 para el Inglés ('en')...

    // 5. Configurar el idioma inicial
    typefix.setLanguage('es');

    console.log("¡TypeFix inicializado correctamente en el navegador!");
    return typefix;
}
```

---

## 4. Conectar TypeFix a un Editor de Texto (Input)

Una vez tienes la instancia `typefix`, solo necesitas interceptar las pulsaciones del usuario y pasárselas al motor.

```javascript
const textarea = document.getElementById('my-editor');
let typefixEngine = await startTypeFix();

textarea.addEventListener('keypress', (event) => {
    // Evitar teclas de control o Enter
    if (event.key.length !== 1) return;

    // Procesar el caracter en TypeFix
    const resultJsonStr = typefixEngine.pushChar(event.key);

    if (resultJsonStr) {
        const result = JSON.parse(resultJsonStr);
        
        // Si TypeFix detectó un cambio de idioma
        if (result.detected_language) {
            console.log(`🌍 Auto-Switch activado: ${result.detected_language.code} (Confianza: ${result.detected_language.confidence})`);
        }

        // Si TypeFix realizó una corrección
        if (result.corrected) {
            console.log(`✨ Corrección automática: ${result.original} -> ${result.corrected}`);
            
            // Lógica para reemplazar la palabra en el textarea
            // (Reemplazar la última palabra escrita por result.corrected)
            replaceLastWordInTextarea(textarea, result.original, result.corrected);
        }
    }
});

function replaceLastWordInTextarea(element, original, corrected) {
    const text = element.value;
    const lastIndex = text.lastIndexOf(original);
    if (lastIndex !== -1) {
        element.value = text.substring(0, lastIndex) + corrected + text.substring(lastIndex + original.length);
    }
}
```

---

## 5. Arquitectura Dinámica y de Interfaz

### Deshacerte del `config.json`
Al construir tu landing page o Dashboard, en lugar de leer un archivo oculto, tú le ofrecerás al usuario botones o *switches* en pantalla (Ej. "Activar Autocorrector", "Idioma: Inglés"). Cuando el usuario haga click, tú simplemente llamarás a `typefix.setLanguage('en')` o inicializarás el objeto con nuevas reglas.

> [!IMPORTANT]
> **Privacidad Total**
> Una de las mayores promesas de valor de TypeFix es la privacidad. Como el procesamiento ocurre al 100% dentro del navegador usando WASM, lo que el usuario escribe **nunca viaja a tus servidores**. Todo sucede localmente.
