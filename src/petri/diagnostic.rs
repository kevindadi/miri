//! Diagnostic formatting for Petri net violations.

use super::cpn::{Marking, Token};
use super::event::PetriEvent;
use std::fmt;

/// A protocol violation detected by the Petri net monitor.
#[derive(Debug, Clone)]
pub struct PetriViolation {
    pub event: PetriEvent,
    pub tid: u32,
    pub object_id: Option<u64>,
    pub span: Option<SpanLike>,
    pub missing_tokens: Vec<(String, Token)>,
    pub current_marking: Marking,
}

/// Simplified span-like info (avoids pulling in rustc_span in petri public API).
#[derive(Debug, Clone)]
pub struct SpanLike {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl fmt::Display for SpanLike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// Format a violation as a human-readable message.
pub fn format_violation(v: &PetriViolation) -> String {
    let mut s = String::new();
    s.push_str("Petri net protocol violation: transition not enabled\n");
    s.push_str(&format!("  Event: {:?}\n", v.event));
    s.push_str(&format!("  Thread ID: {}\n", v.tid));
    if let Some(oid) = v.object_id {
        s.push_str(&format!("  Object ID: {}\n", oid));
    }
    if let Some(ref span) = v.span {
        s.push_str(&format!("  Location: {}\n", span));
    }
    s.push_str("  Missing tokens:\n");
    for (place, token) in &v.missing_tokens {
        s.push_str(&format!("    - {} in place '{}'\n", token, place));
    }
    s.push_str("  Current marking (key places):\n");
    for (place, multiset) in v.current_marking.iter() {
        let tokens: Vec<String> = multiset
            .iter()
            .map(|(t, c)| format!("{} x{}", t, c))
            .collect();
        if !tokens.is_empty() {
            s.push_str(&format!("    {}: [{}]\n", place, tokens.join(", ")));
        }
    }
    s
}
