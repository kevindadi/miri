//! Petri net event definitions for the Miri monitor.
//!
//! Events are emitted at runtime and mapped to CPN transition firings.

use serde::{Deserialize, Serialize};

/// Protocol-layer events emitted during Miri execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum PetriEvent {
    ThreadSpawn {
        parent: u32,
        child: u32,
    },
    ThreadJoin {
        joiner: u32,
        joinee: u32,
    },
    Yield {
        tid: u32,
    },
    Block {
        tid: u32,
        reason: String,
    },
    Wake {
        tid: u32,
    },
    LockAcquire {
        tid: u32,
        lock_id: u64,
    },
    LockRelease {
        tid: u32,
        lock_id: u64,
    },
    AtomicLoad {
        tid: u32,
        loc_id: u64,
        ordering: String,
    },
    AtomicStore {
        tid: u32,
        loc_id: u64,
        ordering: String,
    },
    /// Optional: unsafe memory read (requires petri-unsafe feature).
    UnsafeRead {
        tid: u32,
        region_id: u64,
        size: u64,
    },
    /// Optional: unsafe memory write (requires petri-unsafe feature).
    UnsafeWrite {
        tid: u32,
        region_id: u64,
        size: u64,
    },
}

impl PetriEvent {
    /// Returns the thread ID associated with this event.
    pub fn tid(&self) -> u32 {
        match self {
            PetriEvent::ThreadSpawn { parent, .. } => *parent,
            PetriEvent::ThreadJoin { joiner, .. } => *joiner,
            PetriEvent::Yield { tid } => *tid,
            PetriEvent::Block { tid, .. } => *tid,
            PetriEvent::Wake { tid } => *tid,
            PetriEvent::LockAcquire { tid, .. } => *tid,
            PetriEvent::LockRelease { tid, .. } => *tid,
            PetriEvent::AtomicLoad { tid, .. } => *tid,
            PetriEvent::AtomicStore { tid, .. } => *tid,
            PetriEvent::UnsafeRead { tid, .. } => *tid,
            PetriEvent::UnsafeWrite { tid, .. } => *tid,
        }
    }

    /// Returns the object identifier (lock_id, loc_id, or region_id) if applicable.
    pub fn object_id(&self) -> Option<u64> {
        match self {
            PetriEvent::LockAcquire { lock_id, .. } | PetriEvent::LockRelease { lock_id, .. } => {
                Some(*lock_id)
            }
            PetriEvent::AtomicLoad { loc_id, .. } | PetriEvent::AtomicStore { loc_id, .. } => {
                Some(*loc_id)
            }
            PetriEvent::UnsafeRead { region_id, .. }
            | PetriEvent::UnsafeWrite { region_id, .. } => Some(*region_id),
            _ => None,
        }
    }

    /// Returns the event type name for mapping to transitions.
    pub fn event_type_name(&self) -> &'static str {
        match self {
            PetriEvent::ThreadSpawn { .. } => "ThreadSpawn",
            PetriEvent::ThreadJoin { .. } => "ThreadJoin",
            PetriEvent::Yield { .. } => "Yield",
            PetriEvent::Block { .. } => "Block",
            PetriEvent::Wake { .. } => "Wake",
            PetriEvent::LockAcquire { .. } => "LockAcquire",
            PetriEvent::LockRelease { .. } => "LockRelease",
            PetriEvent::AtomicLoad { .. } => "AtomicLoad",
            PetriEvent::AtomicStore { .. } => "AtomicStore",
            PetriEvent::UnsafeRead { .. } => "UnsafeRead",
            PetriEvent::UnsafeWrite { .. } => "UnsafeWrite",
        }
    }
}
