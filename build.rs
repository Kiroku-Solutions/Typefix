use phf_codegen::Map;
use std::env;
use std::fs;
use std::path::Path;

fn encode_accents(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            'á' => 'A', 'é' => 'E', 'í' => 'I', 'ó' => 'O', 'ú' => 'U',
            'ñ' => 'N', 'ü' => 'W', 'ã' => 'B', 'õ' => 'C', 'ç' => 'D',
            'â' => 'F', 'ê' => 'G', 'ô' => 'J', 'à' => 'L',
            _ => c,
        };
        res.push(mapped);
    }
    res
}

fn strip_accents(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        let stripped = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => c,
        };
        res.push(stripped);
    }
    res
}

fn main() {
    println!("cargo:rerun-if-changed=data/errors");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("static_errors.rs");

    let mut builder = Map::new();
    let mut strings_arena = std::collections::HashMap::new();

    // Directory containing JSON error maps
    let errors_dir = Path::new("data/errors");

    if errors_dir.exists() {
        for entry in fs::read_dir(errors_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_file() && path.extension().unwrap_or_default() == "json" {
                let language = path.file_stem().unwrap().to_str().unwrap();
                let fst_path = format!("data/dictionaries/{}.fst", language);
                
                let fst_map = if Path::new(&fst_path).exists() {
                    let mut data = fs::read(&fst_path).unwrap();
                    if data.len() >= 4 && &data[0..4] == b"TFX1" {
                        data = data[4..].to_vec();
                    }
                    Some(fst::Map::new(data).unwrap())
                } else {
                    None
                };

                // Enforce JSON file size limit (10 MB) to prevent OOM
                if fs::metadata(&path).unwrap().len() > 10_000_000 {
                    panic!("JSON error map is too large: {}", path.display());
                }
                
                // Parse the JSON
                let content = fs::read_to_string(&path).unwrap();
                
                // We'll deserialize to a generic map since we just want the "errors" object
                let json: serde_json::Value = serde_json::from_str(&content).unwrap();
                
                // Extract language
                if let Some(errors) = json.get("errors").and_then(|e| e.as_object()) {
                    for (typo, correction) in errors {
                        if let Some(correction_str) = correction.as_str() {
                            let typo_lower = typo.to_lowercase();
                            
                            if let Some(ref fst) = fst_map {
                                let encoded = encode_accents(&typo_lower);
                                if fst.contains_key(&encoded) {
                                    if strip_accents(&typo_lower) == strip_accents(&correction_str.to_lowercase()) {
                                        // Allow accent-only corrections to pass through
                                    } else {
                                        println!("cargo:warning=Static error key '{}' conflicts with valid dictionary word in {}.fst. It will be excluded.", typo, language);
                                        continue;
                                    }
                                }
                            }
                            
                            let formatted_correction = format!("\"{}\"", correction_str.replace("\"", "\\\""));
                            strings_arena.insert(format!("{}_{}", language, typo_lower), formatted_correction);
                        }
                    }
                }
            }
        }
    }

    for (k, v) in &strings_arena {
        builder.entry(k.as_str(), v.as_str());
    }

    // Write the generated code to OUT_DIR
    let generated_code = format!(
        "/// Map of static errors to their corrections\npub static STATIC_ERRORS: phf::Map<&'static str, &'static str> = \n{};\n",
        builder.build()
    );

    fs::write(&dest_path, generated_code).unwrap();
}
