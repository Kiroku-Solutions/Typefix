use phf_codegen::Map;
use std::env;
use std::fs;
use std::path::Path;

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
                // Parse the JSON
                let content = fs::read_to_string(&path).unwrap();
                
                // We'll deserialize to a generic map since we just want the "errors" object
                let json: serde_json::Value = serde_json::from_str(&content).unwrap();
                
                // Extract language
                if let Some(errors) = json.get("errors").and_then(|e| e.as_object()) {
                    for (typo, correction) in errors {
                        // We must format the correction as a Rust string literal.
                        // We add both lowercase and original typo as a safety measure.
                        if let Some(correction_str) = correction.as_str() {
                            let typo_lower = typo.to_lowercase();
                            let formatted_correction = format!("\"{}\"", correction_str.replace("\"", "\\\""));
                            strings_arena.insert(typo_lower, formatted_correction);
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
        "pub static STATIC_ERRORS: phf::Map<&'static str, &'static str> = \n{};\n",
        builder.build()
    );

    fs::write(&dest_path, generated_code).unwrap();
}
