#![no_main]
use libfuzzer_sys::fuzz_target;
use typefix_wasm_core::TypeFixPipeline;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let pipeline = TypeFixPipeline::simple();
        let _ = pipeline.process_string(s);
    }
});
