# EHR and Legal Systems Integration Guide

This guide covers integrating the TypeFix with Electronic Health Records (EHR) and Legal document management systems.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [EHR Integration](#ehr-integration)
3. [Legal Integration](#legal-integration)
4. [API Reference](#api-reference)
5. [Security Considerations](#security-considerations)
6. [Example Code](#example-code)

---

## Architecture Overview

### High-Level Integration Pattern

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        External System (EHR/Legal)                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────┐    ┌──────────────┐    ┌────────────────────────────┐    │
│  │   Client    │───▶│  Integration  │───▶│     TypeFix            │    │
│  │ Application │    │    Layer      │    │  ┌──────────────────────┐  │    │
│  └─────────────┘    │               │    │  │ TypeFixPipeline         │  │    │
│                     │ • Validation  │    │  │ • CharBuffer         │  │    │
│                     │ • Formatting  │    │  │ • LanguageDetector   │  │    │
│                     │ • Audit Trail │    │  │ • CorrectionEngine   │  │    │
│                     └──────────────┘    │  └──────────────────────┘  │    │
│                                         └────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Integration Patterns

| Pattern | Use Case | Latency Target |
|---------|----------|----------------|
| Real-time | Clinical notes, dictation | < 5ms |
| Batch | Document processing, import | < 100ms/doc |
| On-demand | Spell-check UI, review tools | < 50ms |

---

## EHR Integration

### Overview

Electronic Health Records systems require:

- **HIPAA compliance** for PHI (Protected Health Information) handling
- **Medical terminology** support (ICD-10, SNOMED CT, LOINC)
- **Multi-language** support for diverse patient populations
- **Audit trails** for all text modifications

### HIPAA Compliance Checklist

- [ ] All PHI processed stays within encrypted boundaries
- [ ] No PHI logged or stored in correction cache
- [ ] Audit log for all correction events
- [ ] Business Associate Agreement (BAA) with integration provider

### Configuration for EHR

```json
{
  "integration_mode": "ehr",
  "privacy": {
    "enable_phi_filtering": true,
    "audit_corrections": true,
    "audit_log_path": "/secure/audit/corrections.log",
    "phi_patterns": [
      "^\\d{3}-\\d{2}-\\d{4}$",
      "^\\d{3}\\.\\d{3}\\.\\d{4}$"
    ]
  },
  "medical": {
    "load_medical_dictionary": true,
    "medical_dictionary_path": "data/medical/",
    "custom_terms_path": "data/ehr/custom_terms.json",
    "abbreviation_expansion": true
  },
  "performance": {
    "mode": "realtime",
    "max_latency_ms": 5
  }
}
```

### Medical Dictionary Structure

```json
{
  "language": "en",
  "type": "medical",
  "version": "2024.1",
  "terms": [
    {
      "term": "hypertension",
      "code": "ICD10:I10",
      "category": "diagnosis",
      "frequency": 500000
    },
    {
      "term": "acetaminophen",
      "code": "RXNORM:161",
      "category": "medication",
      "frequency": 300000
    },
    {
      "term": "myocardial infarction",
      "code": "ICD10:I21",
      "synonyms": ["MI", "heart attack"],
      "category": "diagnosis",
      "frequency": 200000
    }
  ]
}
```

---

## Legal Integration

### Overview

Legal document systems require:

- **Precise terminology** preservation (legal citations, case names)
- **Redlining support** for tracking changes
- **Multi-jurisdiction** support (state, federal, international)
- **E-discovery** compatibility

### Configuration for Legal

```json
{
  "integration_mode": "legal",
  "legal": {
    "preserve_citations": true,
    "citation_patterns": [
      "^\\d+\\s+U\\.S\\.\\s+\\d+",
      "^\\d+\\s+F\\.\\d+d\\s+\\d+",
      "^\\d+\\s+S\\.Ct\\.\\s+\\d+"
    ],
    "jurisdiction": "us_federal",
    "strict_mode": true
  },
  "redlining": {
    "track_changes": true,
    "show_suggestions_only": true,
    "preserve_original": true
  }
}
```

### Legal Dictionary Structure

```json
{
  "language": "en",
  "type": "legal",
  "jurisdiction": "us_federal",
  "terms": [
    {
      "term": "pursuant to",
      "category": "boilerplate",
      "frequency": 100000
    },
    {
      "term": "hereby declares",
      "category": "boilerplate",
      "frequency": 80000
    },
    {
      "term": "reasonable doubt",
      "category": "criminal",
      "frequency": 50000
    }
  ]
}
```

---

## API Reference

### Core Functions

#### `init(config: &Config) -> Result<()>`

Initialize the engine with configuration.

```rust
use typefix::{init, Config};

let config = Config::from_file("config.json")?;
init(&config)?;
```

#### `TypeFixPipeline::new(config: PipelineConfig) -> Self`

Create a new processing pipeline.

```rust
use typefix::{TypeFixPipeline, PipelineConfig};

let config = PipelineConfig {
    auto_correct: true,
    detect_language: true,
    buffer_size: 64,
    suggestion_mode: false,
};
let pipeline = TypeFixPipeline::new(config);
```

#### `pipeline.push(ch: char) -> Option<PipelineResult>`

Process a single character.

```rust
if let Some(result) = pipeline.push('h') {
    println!("Word: {}, Corrected: {:?}",
             result.original,
             result.corrected);
}
```

#### `pipeline.process_string(text: &str) -> Vec<PipelineResult>`

Process an entire string.

```rust
let results = pipeline.process_string("Pateint name: John Doe");
for result in results {
    if let Some(corrected) = result.corrected {
        println!("{} -> {}", result.original, corrected);
    }
}
```

### Data Structures

#### `PipelineResult`

```rust
pub struct PipelineResult {
    /// The original word
    pub original: String,
    /// The corrected word (if any)
    pub corrected: Option<String>,
    /// Language detection result (if any)
    pub detected_language: Option<DetectionResult>,
}
```

#### `PipelineEvent`

```rust
pub enum PipelineEvent {
    WordExtracted { word: String },
    LanguageDetected { language: String, confidence: f64 },
    WordCorrected { original: String, corrected: String },
    BufferOverflow { word: String },
}
```

---

## Security Considerations

### Data Privacy

1. **PHI Filtering**: Medical terms containing patient identifiers are never logged
2. **Correction Audit**: All corrections are logged without source content
3. **Memory Safety**: No `unsafe` code in hot paths (#![forbid(unsafe_code)])
4. **Zero-Copy**: Minimal memory allocation in correction pipeline

### Threat Model

| Threat | Mitigation |
|--------|------------|
| Data exfiltration via corrections | No persistent storage of PHI |
| Dictionary poisoning | Signed dictionaries, integrity verification |
| Timing attacks | Constant-time comparison for sensitive terms |
| Memory corruption | 100% safe Rust, no heap allocation in hot path |

### Secure Configuration

```rust
// Production EHR configuration
let config = PipelineConfig {
    auto_correct: true,
    detect_language: true,
    buffer_size: 64,
    suggestion_mode: true,  // User reviews suggestions
};
```

### Audit Logging

```rust
pipeline.on_event(|event| {
    match event {
        PipelineEvent::WordCorrected { original: _, corrected } => {
            // Log WITHOUT original text for HIPAA compliance
            tracing::info!(
                correction_applied = true,
                corrected_term = %corrected,
                timestamp = chrono::Utc::now()
            );
        }
        _ => {}
    }
});
```

---

## Example Code

### EHR Integration Example

```rust
use typefix::{
    TypeFixPipeline, PipelineConfig, PipelineEvent,
    core::{Config, Trie},
    correction::StaticErrorMap,
};
use std::sync::Arc;

/// EHR Integration Handler
pub struct EhrIntegration {
    pipeline: Arc<TypeFixPipeline>,
    audit_log: Arc<Mutex<Vec<CorrectionAudit>>>,
}

#[derive(Debug, Clone)]
pub struct CorrectionAudit {
    pub timestamp: DateTime<Utc>,
    pub corrected_term: String,
    pub document_id: Option<String>,
}

impl EhrIntegration {
    /// Create new EHR integration
    pub fn new(medical_dict_path: &Path) -> Result<Self> {
        let config = PipelineConfig {
            auto_correct: true,
            detect_language: true,
            buffer_size: 64,
            suggestion_mode: false,  // Auto-correct for efficiency
        };

        let pipeline = Arc::new(TypeFixPipeline::new(config));

        // Load medical dictionary
        let medical_dict = Trie::from_json_file(medical_dict_path)?;
        pipeline.add_dictionary("en", medical_dict);

        // Setup audit logging
        let audit_log = Arc::new(Mutex::new(Vec::new()));
        let audit_clone = Arc::clone(&audit_log);

        pipeline.on_event(move |event| {
            if let PipelineEvent::WordCorrected { original: _, corrected } = event {
                let audit = CorrectionAudit {
                    timestamp: Utc::now(),
                    corrected_term: corrected,
                    document_id: None,
                };
                audit_clone.lock().unwrap().push(audit);
            }
        });

        Ok(Self { pipeline, audit_log })
    }

    /// Process clinical note text
    pub fn process_note(&self, note: &str) -> ProcessedNote {
        let results = self.pipeline.process_string(note);

        ProcessedNote {
            original: note.to_string(),
            corrections: results
                .iter()
                .filter_map(|r| r.corrected.clone())
                .collect(),
        }
    }
}

pub struct ProcessedNote {
    pub original: String,
    pub corrections: Vec<String>,
}
```

### Legal Integration Example

```rust
use typefix::{
    TypeFixPipeline, PipelineConfig, PipelineEvent,
    correction::StaticErrorMap,
};
use regex::Regex;
use std::sync::Arc;

/// Legal Integration Handler
pub struct LegalIntegration {
    pipeline: Arc<TypeFixPipeline>,
    citation_pattern: Regex,
    redline_changes: Arc<Mutex<Vec<RedlineEntry>>>,
}

#[derive(Debug, Clone)]
pub struct RedlineEntry {
    pub position: usize,
    pub original: String,
    pub suggestion: String,
    pub accepted: Option<bool>,
}

impl LegalIntegration {
    /// Create new Legal integration
    pub fn new() -> Result<Self> {
        let config = PipelineConfig {
            auto_correct: false,  // Suggestion mode for legal
            detect_language: true,
            buffer_size: 64,
            suggestion_mode: true,  // User must approve corrections
        };

        let pipeline = Arc::new(TypeFixPipeline::new(config));

        // Load legal dictionary
        let legal_dict = Trie::from_json_file("data/legal/en.json")?;
        pipeline.add_dictionary("en", legal_dict);

        // Citation preservation pattern
        let citation_pattern = Regex::new(
            r"\d+\s+[USF]\.\d+d?\s+\d+"
        ).unwrap();

        let redline_changes = Arc::new(Mutex::new(Vec::new()));
        let changes_clone = Arc::clone(&redline_changes);

        pipeline.on_event(move |event| {
            if let PipelineEvent::WordCorrected { original, corrected } = event {
                let entry = RedlineEntry {
                    position: 0,  // Would track actual position
                    original,
                    suggestion: corrected,
                    accepted: None,
                };
                changes_clone.lock().unwrap().push(entry);
            }
        });

        Ok(Self {
            pipeline,
            citation_pattern,
            redline_changes,
        })
    }

    /// Get suggestions for a document (no auto-correction)
    pub fn get_suggestions(&self, text: &str) -> Vec<Suggestion> {
        let results = self.pipeline.process_string(text);

        results
            .into_iter()
            .filter_map(|r| {
                r.corrected.map(|corrected| Suggestion {
                    original: r.original,
                    suggestion: corrected,
                    position: 0,  // Track position in real implementation
                })
            })
            .collect()
    }

    /// Accept a suggestion and update redline
    pub fn accept_suggestion(&self, position: usize) {
        let mut changes = self.redline_changes.lock().unwrap();
        if let Some(entry) = changes.iter_mut().find(|e| e.position == position) {
            entry.accepted = Some(true);
        }
    }

    /// Export redline document
    pub fn export_redline(&self) -> RedlineDocument {
        let changes = self.redline_changes.lock().unwrap();
        RedlineDocument {
            entries: changes.clone(),
            export_timestamp: Utc::now(),
        }
    }
}

pub struct Suggestion {
    pub original: String,
    pub suggestion: String,
    pub position: usize,
}

pub struct RedlineDocument {
    pub entries: Vec<RedlineEntry>,
    pub export_timestamp: DateTime<Utc>,
}
```

### Batch Processing Example

```rust
use typefix::{TypeFixPipeline, PipelineConfig};
use std::path::Path;
use tokio::fs;

/// Batch document processor
pub struct BatchProcessor {
    pipeline: Arc<TypeFixPipeline>,
    max_batch_size: usize,
}

impl BatchProcessor {
    pub fn new() -> Self {
        let config = PipelineConfig {
            auto_correct: true,
            detect_language: true,
            buffer_size: 128,  // Larger buffer for batch
            suggestion_mode: false,
        };

        Self {
            pipeline: Arc::new(TypeFixPipeline::new(config)),
            max_batch_size: 1000,
        }
    }

    /// Process multiple documents in batch
    pub async fn process_batch(
        &self,
        paths: Vec<PathBuf>,
    ) -> Vec<BatchResult> {
        let mut results = Vec::new();

        for path in paths.into_iter().take(self.max_batch_size) {
            match self.process_document(&path).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(BatchResult {
                        path,
                        status: BatchStatus::Failed(e.to_string()),
                        corrections_applied: 0,
                    });
                }
            }
        }

        results
    }

    async fn process_document(&self, path: &Path) -> Result<BatchResult> {
        let content = fs::read_to_string(path).await?;

        let start = std::time::Instant::now();
        let results = self.pipeline.process_string(&content);
        let elapsed = start.elapsed();

        let corrections = results.iter()
            .filter(|r| r.corrected.is_some())
            .count();

        Ok(BatchResult {
            path: path.to_path_buf(),
            status: BatchStatus::Completed,
            corrections_applied: corrections,
        })
    }
}

pub struct BatchResult {
    pub path: PathBuf,
    pub status: BatchStatus,
    pub corrections_applied: usize,
}

pub enum BatchStatus {
    Completed,
    Failed(String),
}
```

---

## Testing Integration

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ehr_medical_term_correction() {
        let integration = EhrIntegration::new(
            Path::new("data/medical/en.json")
        ).unwrap();

        let result = integration.process_note(
            "Pateint has dianetes type 2"
        );

        assert!(result.corrections.contains(&"patient".to_string()));
        assert!(result.corrections.contains(&"diabetes".to_string()));
    }

    #[test]
    fn test_legal_citation_preservation() {
        let integration = LegalIntegration::new().unwrap();

        let text = "See Roe v. Wade, 410 U.S. 113 (1973)";
        let suggestions = integration.get_suggestions(text);

        // Legal citations should not be flagged
        assert!(suggestions.is_empty());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_batch_processing() {
    let temp_dir = tempdir().unwrap();
    let processor = BatchProcessor::new();

    // Create test documents
    for i in 0..10 {
        let path = temp_dir.path().join(format!("doc_{}.txt", i));
        fs::write(&path, "Sample text with erros").await.unwrap();
    }

    let paths: Vec<_> = (0..10)
        .map(|i| temp_dir.path().join(format!("doc_{}.txt", i)))
        .collect();

    let results = processor.process_batch(paths).await;

    assert_eq!(results.len(), 10);
    assert!(results.iter().all(|r| matches!(r.status, BatchStatus::Completed)));
}
```