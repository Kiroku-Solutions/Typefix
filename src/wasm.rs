use crate::core::Dict;
use crate::correction::StaticErrorMap;
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
    pub fn new(auto_correct: bool, enable_distance: bool, max_distance: usize) -> Self {
        console_error_panic_hook::set_once();
        let config = PipelineConfig {
            auto_correct,
            detect_language: false,
            buffer_size: 64,
            suggestion_mode: false,
        };

        let pipeline = TypeFixPipeline::new(config);



        Self { pipeline }
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

    /// Load static errors from a JSON string
    #[wasm_bindgen(js_name = loadStaticErrors)]
    pub fn load_static_errors(&self, lang: &str, json_str: &str) -> Result<(), JsValue> {
        let map = StaticErrorMap::from_json_str(lang, json_str)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse static errors JSON: {}", e)))?;
        self.pipeline.add_error_map(lang, Arc::new(map));
        Ok(())
    }

    /// Process a character input
    /// Returns a JSON string of the PipelineEvent if an event was generated, or null.
    #[wasm_bindgen(js_name = pushChar)]
    pub fn push_char(&self, ch: char) -> Option<String> {
        if let Some(result) = self.pipeline.push(ch) {
            return serde_json::to_string(&result).ok();
        }
        None
    }

    /// Process an entire string statelessly
    /// Returns a JSON array of PipelineEvents
    #[wasm_bindgen(js_name = processString)]
    pub fn process_string(&self, text: &str) -> String {
        let text = if text.len() > 50_000 {
            &text[..50_000]
        } else {
            text
        };
        
        let start_time = js_sys::Date::now();
        let mut results = Vec::new();
        
        for ch in text.chars() {
            if js_sys::Date::now() - start_time > 100.0 {
                break; // 100ms timeout exceeded
            }
            if let Some(result) = self.pipeline.push(ch) {
                results.push(result);
            }
        }
        
        if js_sys::Date::now() - start_time <= 100.0 {
            let remaining = self.pipeline.buffer_contents();
            if !remaining.is_empty() {
                let result = self.pipeline.get_suggestions(&remaining);
                let corrected = if !result.is_empty() {
                    Some(result[0].word.clone())
                } else {
                    None
                };
                results.push(crate::pipeline::PipelineResult {
                    original: remaining,
                    corrected,
                    detected_language: None,
                });
            }
        }
        
        serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string())
    }

    /// Clear the internal buffer
    #[wasm_bindgen(js_name = clear)]
    pub fn clear(&self) {
        self.pipeline.clear();
    }
}
