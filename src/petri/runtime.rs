//! Petri net runtime: loads net definition, maps events to transitions, maintains marking.

use rustc_data_structures::fx::FxHashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};

use super::config::PetriConfig;
use super::cpn::{ArcSpec, ArcTokenPattern, CpnEngine, Marking, Token, Transition};
use super::diagnostic::{format_violation, PetriViolation, SpanLike};
use super::event::PetriEvent;

/// JSON structure for loading net definition.
#[derive(Debug, serde::Deserialize)]
struct PetriNetDef {
    #[serde(default)]
    #[allow(dead_code)]
    places: Vec<String>,
    transitions: FxHashMap<String, TransitionDef>,
    #[serde(default)]
    event_mapping: FxHashMap<String, String>,
    #[serde(default)]
    initial_marking: FxHashMap<String, Vec<serde_json::Value>>,
}

#[derive(Debug, serde::Deserialize)]
struct TransitionDef {
    pre: Vec<ArcDef>,
    post: Vec<ArcDef>,
}

#[derive(Debug, serde::Deserialize)]
struct ArcDef {
    place: String,
    #[serde(flatten)]
    token: ArcTokenDef,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum ArcTokenDef {
    Variable { variable: String },
    Concrete { kind: String, value: u64 },
    Unit {},
}

impl ArcTokenDef {
    fn to_arc_spec(&self, place: &str) -> ArcSpec {
        let token = match self {
            ArcTokenDef::Variable { variable } => ArcTokenPattern::Variable(variable.clone()),
            ArcTokenDef::Concrete { kind, value } => {
                let t = match kind.as_str() {
                    "Lock" => Token::Lock(*value),
                    "Loc" => Token::Loc(*value),
                    "Tid" => Token::Tid(*value as u32),
                    "Region" => Token::Region(*value),
                    _ => Token::Unit,
                };
                ArcTokenPattern::Concrete(t)
            }
            ArcTokenDef::Unit {} => ArcTokenPattern::Concrete(Token::Unit),
        };
        ArcSpec {
            place: place.to_string(),
            token,
        }
    }
}

/// Parse a token from JSON: ["Lock", 1] or {"kind":"Lock","value":1}.
fn parse_initial_token(v: &serde_json::Value) -> Token {
    if let Some(arr) = v.as_array() {
        if arr.len() >= 2 {
            let kind = arr[0].as_str().unwrap_or("");
            let value = arr[1].as_u64().unwrap_or(0);
            return match kind {
                "Lock" => Token::Lock(value),
                "Loc" => Token::Loc(value),
                "Tid" => Token::Tid(value as u32),
                "Region" => Token::Region(value),
                _ => Token::Unit,
            };
        }
    }
    if let Some(obj) = v.as_object() {
        let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let value = obj.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
        return match kind {
            "Lock" => Token::Lock(value),
            "Loc" => Token::Loc(value),
            "Tid" => Token::Tid(value as u32),
            "Region" => Token::Region(value),
            _ => Token::Unit,
        };
    }
    Token::Unit
}

/// Runtime state for the Petri net monitor.
pub struct PetriRuntime {
    engine: CpnEngine,
    config: PetriConfig,
    event_mapping: FxHashMap<String, String>,
    initial_marking: Marking,
    seen_markings: HashSet<u64>,
    log_file: Option<BufWriter<File>>,
}

impl PetriRuntime {
    /// Load runtime from config file.
    pub fn load(config: PetriConfig) -> Result<Self, String> {
        let contents = std::fs::read_to_string(&config.config_path)
            .map_err(|e| format!("Failed to read Petri config {}: {}", config.config_path.display(), e))?;
        let def: PetriNetDef = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse Petri config: {}", e))?;

        let mut engine = CpnEngine::new();

        let event_mapping = def.event_mapping.clone();

        for (tid, tdef) in &def.transitions {
            let pre: Vec<ArcSpec> = tdef
                .pre
                .iter()
                .map(|a| a.token.to_arc_spec(&a.place))
                .collect();
            let post: Vec<ArcSpec> = tdef
                .post
                .iter()
                .map(|a| a.token.to_arc_spec(&a.place))
                .collect();
            engine.add_transition(Transition {
                id: tid.clone(),
                pre,
                post,
            });
        }

        let mut marking = Marking::new();
        for (place, tokens) in &def.initial_marking {
            for v in tokens {
                let token = parse_initial_token(v);
                marking.get_or_insert(place).add(token, 1);
            }
        }
        let initial_marking = marking.clone();
        engine.set_initial_marking(marking);

        let log_file = config.log_path.as_ref().and_then(|p| {
            File::create(p)
                .ok()
                .map(|f| BufWriter::new(f))
        });

        Ok(Self {
            engine,
            config,
            event_mapping,
            initial_marking,
            seen_markings: HashSet::new(),
            log_file,
        })
    }

    /// Process an event. Returns Err(PetriViolation) if transition is not enabled.
    pub fn on_event(
        &mut self,
        e: PetriEvent,
        span: Option<SpanLike>,
    ) -> Result<(), PetriViolation> {
        let transition_id = match self.config.config_path.extension() {
            Some(_) => self.get_transition_for_event(&e),
            None => self.get_transition_for_event(&e),
        };

        let transition_id = match transition_id {
            Some(t) => t,
            None => return Ok(()), // No mapping for this event type, skip
        };

        let binding = self.make_binding(&e);

        // Lazy init: for LockAcquire, ensure the lock token exists in "free" if this is acquire.
        if transition_id == "acquire" {
            if let Some(&Token::Lock(lock_id)) = binding.get("L") {
                let marking = self.engine.marking_mut();
                let free = marking.get("free");
                if free.map_or(true, |m| !m.contains(&Token::Lock(lock_id), 1)) {
                    marking.get_or_insert("free").add(Token::Lock(lock_id), 1);
                }
            }
        }
        let result = self.engine.fire(&transition_id, &binding);

        if let Err(ref not_enabled) = result {
            let violation = PetriViolation {
                event: e.clone(),
                tid: e.tid(),
                object_id: e.object_id(),
                span,
                missing_tokens: not_enabled.missing.clone(),
                current_marking: self.engine.marking().clone(),
            };
            return Err(violation);
        }

        if self.config.print_marking_on_each_event {
            eprintln!(
                "[Petri] After {:?}: marking hash = {}",
                e.event_type_name(),
                self.engine.marking_hash()
            );
        }

        if let Some(ref mut w) = self.log_file {
            let _ = writeln!(
                w,
                "{}",
                serde_json::json!({
                    "event": e,
                    "marking_hash": self.engine.marking_hash()
                })
            );
            let _ = w.flush();
        }

        Ok(())
    }

    fn get_transition_for_event(&self, e: &PetriEvent) -> Option<String> {
        let type_name = e.event_type_name();
        self.event_mapping
            .get(type_name)
            .cloned()
            .or_else(|| match type_name {
                "LockAcquire" => Some("acquire".to_string()),
                "LockRelease" => Some("release".to_string()),
                _ => None,
            })
    }

    fn make_binding(&self, e: &PetriEvent) -> FxHashMap<String, Token> {
        let mut binding = FxHashMap::default();
        binding.insert("tid".to_string(), Token::Tid(e.tid()));
        match e {
            PetriEvent::LockAcquire { lock_id, .. } | PetriEvent::LockRelease { lock_id, .. } => {
                binding.insert("L".to_string(), Token::Lock(*lock_id));
            }
            PetriEvent::AtomicLoad { loc_id, .. } | PetriEvent::AtomicStore { loc_id, .. } => {
                binding.insert("loc".to_string(), Token::Loc(*loc_id));
            }
            _ => {}
        }
        binding
    }

    /// Record execution end (for GenMC). Returns marking hash and whether it was new.
    pub fn record_execution_end(&mut self) -> (u64, bool) {
        let hash = self.engine.marking_hash();
        let is_new = self.seen_markings.insert(hash);
        (hash, is_new)
    }

    /// Reset for a new execution (e.g. next GenMC run).
    pub fn reset(&mut self) {
        self.seen_markings.clear();
        self.engine.set_initial_marking(self.initial_marking.clone());
    }

    pub fn format_violation(v: &PetriViolation) -> String {
        format_violation(v)
    }

    pub fn marking_hash(&self) -> u64 {
        self.engine.marking_hash()
    }

    pub fn seen_markings_count(&self) -> usize {
        self.seen_markings.len()
    }

    pub fn fail_fast(&self) -> bool {
        self.config.fail_fast
    }

    pub fn config(&self) -> &PetriConfig {
        &self.config
    }
}
