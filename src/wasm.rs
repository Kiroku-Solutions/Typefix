use crate::core::Dict;
use crate::language::StopwordsSet;
use crate::pipeline::{PipelineConfig, TypeFixPipeline};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct TypeFixWeb {
    pipeline: TypeFixPipeline,
}

#[wasm_bindgen]
impl TypeFixWeb {
    #[wasm_bindgen(constructor)]
    pub fn new(
        auto_correct: bool,
        detect_language: bool,
        buffer_size: usize,
    ) -> Self {
        // We use default console_error_panic_hook for better errors in JS
        // #[cfg(feature = "console_error_panic_hook")]
        // console_error_panic_hook::set_once();

        let config = PipelineConfig {
            auto_correct,
            detect_language,
            buffer_size,
            suggestion_mode: false,
        };

        Self {
            pipeline: TypeFixPipeline::new(config),
        }
    }

    /// Set the current language
    #[wasm_bindgen(js_name = setLanguage)]
    pub fn set_language(&self, lang: &str) {
        self.pipeline.set_language(lang);
    }

    /// Load an FST dictionary from a Uint8Array
    #[wasm_bindgen(js_name = loadDictionary)]
    pub fn load_dictionary(&self, lang: &str, bytes: &[u8]) -> Result<(), JsValue> {
        let dict = Dict::from_bytes(bytes.to_vec())
            .map_err(|e| JsValue::from_str(&format!("Failed to load dictionary: {}", e)))?;
        self.pipeline.add_dictionary(lang, Arc::new(dict));
        Ok(())
    }

    /// Load stopwords from a JSON string
    #[wasm_bindgen(js_name = loadStopwords)]
    pub fn load_stopwords(&self, lang: &str, json_str: &str) -> Result<(), JsValue> {
        let stopwords_vec: Vec<String> = serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse stopwords JSON: {}", e)))?;
        
        let mut stopwords_set = StopwordsSet::new();
        for word in stopwords_vec {
            stopwords_set.insert(&word);
        }
        
        self.pipeline.add_stopwords(lang, Arc::new(stopwords_set));
        Ok(())
    }

    /// Process a character input
    /// Returns a JSON string of the PipelineEvent if an event was generated, or null.
    #[wasm_bindgen(js_name = pushChar)]
    pub fn push_char(&self, ch: char) -> Option<String> {
        if let Some(result) = self.pipeline.push(ch) {
            // We can return the correction result as JSON
            // Result is a struct, we could define a JS representation or just serialize it
            // For simplicity, we return JSON string.
            // Wait, pipeline.push() returns `PipelineResult` which has original, corrected, detected_language.
            // Let's manually construct a JSON string or use serde.
            let mut json = String::new();
            json.push_str("{");
            json.push_str(&format!("\"original\": \"{}\"", result.original));
            
            if let Some(corrected) = result.corrected {
                json.push_str(&format!(", \"corrected\": \"{}\"", corrected));
            } else {
                json.push_str(", \"corrected\": null");
            }

            if let Some(lang) = result.detected_language {
                json.push_str(&format!(
                    ", \"detected_language\": {{\"code\": \"{}\", \"confidence\": {}}}",
                    lang.language, lang.confidence
                ));
            } else {
                json.push_str(", \"detected_language\": null");
            }
            json.push_str("}");
            return Some(json);
        }
        None
    }
}
