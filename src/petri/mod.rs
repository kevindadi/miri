//! Colored Petri Net monitor for Miri.
//!
//! This module provides an online monitor that maps protocol-layer events
//! (lock/unlock, atomic ops, thread spawn/join, etc.) to a Colored Petri Net
//! and detects protocol violations when a transition is not enabled.

pub mod config;
pub mod cpn;
pub mod diagnostic;
pub mod event;
pub mod hooks;
pub mod runtime;

pub use self::config::PetriConfig;
pub use self::cpn::{CpnEngine, Marking, NotEnabled, Token};
pub use self::diagnostic::{format_violation, PetriViolation, SpanLike};
pub use self::event::PetriEvent;
pub use self::runtime::PetriRuntime;
