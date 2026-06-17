//! Memory profiling utilities for TypeFix
//!
//! Provides memory usage tracking and benchmarking utilities.

use std::collections::HashMap;

/// Memory statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Resident Set Size in bytes
    pub rss: usize,
    /// Heap usage in bytes (estimated)
    pub heap_used: usize,
    /// Virtual memory size in bytes
    pub virtual_size: usize,
}

impl MemoryStats {
    /// Create new empty memory stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Get memory usage as MB
    pub fn as_mb(&self) -> f64 {
        self.rss as f64 / (1024.0 * 1024.0)
    }

    /// Check if under memory limit
    pub fn is_under_limit(&self, limit_mb: f64) -> bool {
        self.as_mb() < limit_mb
    }
}

/// Get current memory usage of the process
///
/// Returns RSS (Resident Set Size) which is the actual physical memory used.
#[cfg(target_os = "windows")]
pub fn get_memory_usage() -> MemoryStats {
    use std::mem::MaybeUninit;

    // On Windows, use the Windows API
    #[derive(Default)]
    struct PROCESS_MEMORY_COUNTERS {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "psapi")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            ppsmemCounters: *mut PROCESS_MEMORY_COUNTERS,
            cb: u32,
        ) -> i32;
    }

    let mut counters = MaybeUninit::<PROCESS_MEMORY_COUNTERS>::uninit();
    let mut stats = MemoryStats::new();

    unsafe {
        if GetProcessMemoryInfo(
            GetCurrentProcess(),
            counters.as_mut_ptr(),
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0
        {
            let counters = counters.assume_init();
            stats.rss = counters.working_set_size;
            stats.virtual_size = counters.pagefile_usage;
        }
    }

    stats
}

/// Get current memory usage of the process (Unix implementation)
#[cfg(target_os = "linux")]
pub fn get_memory_usage() -> MemoryStats {
    use std::fs;

    let mut stats = MemoryStats::new();

    // Read /proc/self/status for memory info
    if let Ok(content) = fs::read_to_string("/proc/self/status") {
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                // VmRSS is in kB
                if let Some(kb) = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    stats.rss = kb * 1024;
                }
            } else if line.starts_with("VmSize:") {
                if let Some(kb) = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    stats.virtual_size = kb * 1024;
                }
            }
        }
    }

    stats
}

/// Get current memory usage of the process (macOS implementation)
#[cfg(target_os = "macos")]
pub fn get_memory_usage() -> MemoryStats {
    use std::fs;

    let mut stats = MemoryStats::new();

    // On macOS, use ps command
    if let Ok(output) = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
    {
        if let Ok(rss_str) = String::from_utf8(output.stdout) {
            if let Some(rss) = rss_str.trim().parse::<usize>().ok() {
                stats.rss = rss * 4096; // ps returns pages on macOS
            }
        }
    }

    stats
}

/// Memory tracker for benchmarking
#[derive(Debug, Clone, Default)]
pub struct MemoryTracker {
    /// Initial memory baseline
    baseline: Option<MemoryStats>,
    /// Peak memory observed
    peak: MemoryStats,
    /// Samples collected
    samples: Vec<MemoryStats>,
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Start tracking with baseline
    pub fn start(&mut self) {
        self.baseline = Some(get_memory_usage());
        self.peak = MemoryStats::new();
        self.samples.clear();
    }

    /// Take a memory sample
    pub fn sample(&mut self) {
        let current = get_memory_usage();
        self.samples.push(current.clone());

        if current.rss > self.peak.rss {
            self.peak = current;
        }
    }

    /// Get memory increase since baseline
    pub fn increase(&self) -> Option<MemoryStats> {
        let baseline = self.baseline.as_ref()?;
        let current = self.samples.last()?;

        Some(MemoryStats {
            rss: current.rss.saturating_sub(baseline.rss),
            heap_used: current.heap_used.saturating_sub(baseline.heap_used),
            virtual_size: current
                .virtual_size
                .saturating_sub(baseline.virtual_size),
        })
    }

    /// Get peak memory
    pub fn peak_memory(&self) -> MemoryStats {
        self.peak.clone()
    }

    /// Get all samples
    pub fn samples(&self) -> &[MemoryStats] {
        &self.samples
    }

    /// Get average memory
    pub fn average(&self) -> Option<MemoryStats> {
        if self.samples.is_empty() {
            return None;
        }

        let sum: MemoryStats = self.samples.iter().fold(MemoryStats::new(), |acc, s| {
            MemoryStats {
                rss: acc.rss + s.rss,
                heap_used: acc.heap_used + s.heap_used,
                virtual_size: acc.virtual_size + s.virtual_size,
            }
        });

        let len = self.samples.len() as f64;
        Some(MemoryStats {
            rss: (sum.rss as f64 / len) as usize,
            heap_used: (sum.heap_used as f64 / len) as usize,
            virtual_size: (sum.virtual_size as f64 / len) as usize,
        })
    }
}

/// Memory benchmark result
#[derive(Debug, Clone)]
pub struct MemoryBenchmarkResult {
    /// Initial memory in bytes
    pub initial_mb: f64,
    /// Final memory in bytes
    pub final_mb: f64,
    /// Peak memory in bytes
    pub peak_mb: f64,
    /// Memory increase in bytes
    pub increase_mb: f64,
    /// Whether under limit
    pub under_limit: bool,
    /// Limit that was checked
    pub limit_mb: f64,
}

impl MemoryBenchmarkResult {
    /// Create a new result
    pub fn new(initial: MemoryStats, final_stats: MemoryStats, peak: MemoryStats, limit_mb: f64) -> Self {
        let under_limit = peak.as_mb() < limit_mb;

        Self {
            initial_mb: initial.as_mb(),
            final_mb: final_stats.as_mb(),
            peak_mb: peak.as_mb(),
            increase_mb: (final_stats.rss as f64 - initial.rss as f64) / (1024.0 * 1024.0),
            under_limit,
            limit_mb,
        }
    }

    /// Summary string
    pub fn summary(&self) -> String {
        format!(
            "Memory: initial={:.2}MB, final={:.2}MB, peak={:.2}MB, increase={:.2}MB, under_10MB={}",
            self.initial_mb, self.final_mb, self.peak_mb, self.increase_mb, self.under_limit
        )
    }
}

/// Track dictionary memory usage
#[derive(Debug, Clone, Default)]
pub struct DictionaryMemoryTracker {
    dictionaries: HashMap<String, usize>, // lang -> estimated_size
}

impl DictionaryMemoryTracker {
    /// Create new tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dictionary
    pub fn add_dictionary(&mut self, lang: &str, word_count: usize, avg_word_len: usize) {
        // Rough estimate: each word entry ~ (avg_len + overhead) bytes
        // Trie node overhead ~ 48 bytes + children hashmap
        let estimated = word_count * (avg_word_len + 64);
        self.dictionaries.insert(lang.to_string(), estimated);
    }

    /// Get total estimated memory
    pub fn total_memory(&self) -> usize {
        self.dictionaries.values().sum()
    }

    /// Get memory as MB
    pub fn total_memory_mb(&self) -> f64 {
        self.total_memory() as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats_mb() {
        let stats = MemoryStats {
            rss: 10 * 1024 * 1024, // 10 MB
            heap_used: 5 * 1024 * 1024,
            virtual_size: 20 * 1024 * 1024,
        };

        assert_eq!(stats.as_mb(), 10.0);
        assert!(stats.is_under_limit(15.0));
        assert!(!stats.is_under_limit(5.0));
    }

    #[test]
    fn test_memory_tracker() {
        let mut tracker = MemoryTracker::new();
        tracker.start();
        tracker.sample();
        tracker.sample();

        assert!(tracker.increase().is_some());
        assert_eq!(tracker.samples().len(), 2);
    }

    #[test]
    fn test_dictionary_memory_tracker() {
        let mut tracker = DictionaryMemoryTracker::new();
        tracker.add_dictionary("en", 50000, 6);
        tracker.add_dictionary("es", 50000, 6);

        let total = tracker.total_memory();
        assert!(total > 0);
        // ~50K words * 6 chars * 2 languages = ~600K + overhead
        assert!(total < 10_000_000); // Should be under 10MB
    }
}
