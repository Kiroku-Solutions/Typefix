use typefix::core::Dict;
use std::path::Path;

fn main() {
    let json_path = Path::new("data/dictionaries/es.json");
    let fst_path = Path::new("data/dictionaries/es.fst");
    println!("Compiling {} to {}...", json_path.display(), fst_path.display());
    
    Dict::compile_json_to_fst(json_path, fst_path).unwrap();
    println!("Done!");
}
