# TypeFix WebAssembly (WASM) Integration Guide

Welcome to the TypeFix web integration guide! With this guide, you will be able to bring the power of the ultra-fast correction engine and intelligent auto-switch directly to your users' browsers, without the need for a backend or API calls.

TypeFix compiles to WebAssembly (WASM), allowing your JavaScript application (React, Vue, Vanilla) to initialize a text processing pipeline with 0 milliseconds of latency.

---

## 1. Installation and Compilation

First, we need to package TypeFix as an NPM package. To do this, we use `wasm-pack`:

```bash
# Install wasm-pack if you don't have it
cargo install wasm-pack

# Compile the package for the web
wasm-pack build --target web --out-name typefix-wasm --out-dir pkg
```

This will generate a `/pkg` folder with the `.js` and `.wasm` files ready to use in your web project.

---

## 2. Dictionary and Stopwords Injection

In the web environment, the local `config.json` file does not exist. Instead, the Front-End is responsible for downloading resources (FST Dictionaries and JSON lists) using `fetch()` calls.

### Static Files Structure
Upload your language files to the public directory of your server or CDN:
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
> **Performance**
> `.fst` files are extremely compact binaries. A dictionary of 600,000 words in Spanish typically weighs only 1.5MB. They will load instantly on the web!

---

## 3. Practical Initialization Example (JavaScript)

The following code demonstrates how to import the package, download languages, and listen to a text area:

```javascript
import init, { TypeFixWeb } from './pkg/typefix_wasm.js';

async function startTypeFix() {
    // 1. Initialize the WASM engine
    await init();

    // 2. Create the instance. (auto_correct=true, detect_language=true, buffer_size=64)
    const typefix = new TypeFixWeb(true, true, 64);

    // 3. Download and inject the Spanish dictionary
    const esDictResponse = await fetch('/dictionaries/es.fst');
    const esDictBuffer = await esDictResponse.arrayBuffer();
    typefix.loadDictionary('es', new Uint8Array(esDictBuffer));

    // 4. Download and inject Stopwords (for Auto-Switch)
    const esStopwordsResponse = await fetch('/stopwords/es.json');
    const esStopwordsJson = await esStopwordsResponse.text();
    typefix.loadStopwords('es', esStopwordsJson);
    
    // Repeat steps 3 and 4 for English ('en')...

    // 5. Set the initial language
    typefix.setLanguage('es');

    console.log("TypeFix successfully initialized in the browser!");
    return typefix;
}
```

---

## 4. Connecting TypeFix to a Text Editor (Input)

Once you have the `typefix` instance, you only need to intercept user keystrokes and pass them to the engine.

```javascript
const textarea = document.getElementById('my-editor');
let typefixEngine = await startTypeFix();

textarea.addEventListener('keypress', (event) => {
    // Ignore control keys or Enter
    if (event.key.length !== 1) return;

    // Process the character in TypeFix
    const resultJsonStr = typefixEngine.pushChar(event.key);

    if (resultJsonStr) {
        const result = JSON.parse(resultJsonStr);
        
        // If TypeFix detected a language change
        if (result.detected_language) {
            console.log(`🌍 Auto-Switch activated: ${result.detected_language.code} (Confidence: ${result.detected_language.confidence})`);
        }

        // If TypeFix performed a correction
        if (result.corrected) {
            console.log(`✨ Automatic correction: ${result.original} -> ${result.corrected}`);
            
            // Logic to replace the word in the textarea
            // (Replace the last typed word with result.corrected)
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

## 5. Dynamic Interface Architecture

### Getting rid of `config.json`
When building your landing page or Dashboard, instead of reading a hidden file, you will offer the user buttons or switches on the screen (e.g., "Enable Auto-correct", "Language: English"). When the user clicks, you will simply call `typefix.setLanguage('en')` or initialize the object with new rules.

> [!IMPORTANT]
> **Total Privacy**
> One of TypeFix's greatest value propositions is privacy. Since processing occurs 100% within the browser using WASM, what the user types **never travels to your servers**. Everything happens locally.
