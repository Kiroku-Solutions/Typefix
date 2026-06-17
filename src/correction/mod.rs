//! Correction module - typo correction engine

pub mod damerau;
pub mod engine;
pub mod static_map;

pub use damerau::DamerauLevenshtein;
pub use engine::CorrectionEngine;
pub use static_map::StaticErrorMap;
