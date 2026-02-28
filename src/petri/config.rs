//! Configuration for the Petri net monitor.

use std::path::PathBuf;

/// Configuration for the Petri net monitor.
#[derive(Debug, Clone)]
pub struct PetriConfig {
    /// Path to the Petri net definition (JSON).
    pub config_path: PathBuf,
    /// Optional path to log events and markings (NDJSON).
    pub log_path: Option<PathBuf>,
    /// If true, report violation and abort immediately. If false, log and continue.
    pub fail_fast: bool,
    /// If true, print current marking after each event (for debugging).
    pub print_marking_on_each_event: bool,
}

impl PetriConfig {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            log_path: None,
            fail_fast: true,
            print_marking_on_each_event: false,
        }
    }

    pub fn with_log_path(mut self, path: PathBuf) -> Self {
        self.log_path = Some(path);
        self
    }

    pub fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    pub fn with_print_marking_on_each_event(mut self, v: bool) -> Self {
        self.print_marking_on_each_event = v;
        self
    }
}
