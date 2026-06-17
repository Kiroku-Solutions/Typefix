//! Language detection module

pub mod detector;
pub mod stopwords;

pub use detector::LanguageDetector;
pub use stopwords::StopwordsTrie;
