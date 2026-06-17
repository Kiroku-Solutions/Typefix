//! Language detection module

pub mod detector;
pub mod resolver;
pub mod stopwords;

pub use detector::LanguageDetector;
pub use resolver::{resolve_language, LanguageSource, ResolvedLanguage};
pub use stopwords::StopwordsTrie;
