//! Colored Petri Net (CPN) engine for protocol monitoring.
//!
//! A minimal research prototype supporting place/transition nets with
//! colored tokens.

use rustc_data_structures::fx::FxHashMap;
use std::fmt;

/// Place identifier.
pub type PlaceId = String;

/// Transition identifier.
pub type TransitionId = String;

/// Token types for colored Petri nets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    Tid(u32),
    Lock(u64),
    Loc(u64),
    Region(u64),
    Unit,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Tid(t) => write!(f, "Tid({})", t),
            Token::Lock(l) => write!(f, "Lock({})", l),
            Token::Loc(l) => write!(f, "Loc({})", l),
            Token::Region(r) => write!(f, "Region({})", r),
            Token::Unit => write!(f, "Unit"),
        }
    }
}

/// Multiset of tokens (bag of tokens per place).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Multiset(FxHashMap<Token, usize>);

impl Multiset {
    pub fn new() -> Self {
        Self(FxHashMap::default())
    }

    pub fn add(&mut self, token: Token, count: usize) {
        *self.0.entry(token).or_insert(0) += count;
    }

    pub fn remove(&mut self, token: &Token, count: usize) -> bool {
        let entry = self.0.get_mut(token);
        match entry {
            Some(n) if *n >= count => {
                *n -= count;
                if *n == 0 {
                    self.0.remove(token);
                }
                true
            }
            _ => false,
        }
    }

    pub fn contains(&self, token: &Token, count: usize) -> bool {
        self.0.get(token).map_or(false, |n| *n >= count)
    }

    pub fn count(&self, token: &Token) -> usize {
        *self.0.get(token).unwrap_or(&0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Token, &usize)> {
        self.0.iter()
    }
}

/// Arc specification: (place_id, token_pattern).
/// Token patterns can be variable names (e.g. "L" for Lock) or "tid" for thread.
#[derive(Debug, Clone)]
pub struct ArcSpec {
    pub place: PlaceId,
    pub token: ArcTokenPattern,
}

/// Token pattern in arc: either a concrete token or a variable to bind.
#[derive(Debug, Clone)]
pub enum ArcTokenPattern {
    Concrete(Token),
    Variable(String),
}

/// Transition definition with pre and post arcs.
#[derive(Debug, Clone)]
pub struct Transition {
    pub id: TransitionId,
    pub pre: Vec<ArcSpec>,
    pub post: Vec<ArcSpec>,
}

/// Error when a transition cannot fire.
#[derive(Debug, Clone)]
pub struct NotEnabled {
    pub transition: TransitionId,
    pub missing: Vec<(PlaceId, Token)>,
}

impl fmt::Display for NotEnabled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Transition {} not enabled. Missing tokens:", self.transition)?;
        for (place, token) in &self.missing {
            write!(f, " {} in place '{}'", token, place)?;
        }
        Ok(())
    }
}

/// Marking: place -> multiset of tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marking(FxHashMap<PlaceId, Multiset>);

impl Marking {
    pub fn new() -> Self {
        Self(FxHashMap::default())
    }

    pub fn add_token(&mut self, place: &str, token: Token, count: usize) {
        self.0.entry(place.to_string()).or_default().add(token, count);
    }

    pub fn get(&self, place: &str) -> Option<&Multiset> {
        self.0.get(place)
    }

    pub fn get_mut(&mut self, place: &str) -> Option<&mut Multiset> {
        self.0.get_mut(place)
    }

    pub fn get_or_insert(&mut self, place: &str) -> &mut Multiset {
        self.0.entry(place.to_string()).or_default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PlaceId, &Multiset)> {
        self.0.iter()
    }

    /// Compute a hash for coverage tracking.
    pub fn hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        let mut places: Vec<_> = self.0.keys().collect();
        places.sort();
        for p in places {
            p.hash(&mut hasher);
            let m = self.0.get(p).unwrap();
            let mut tokens: Vec<_> = m.iter().collect();
            tokens.sort_by(|a, b| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)));
            for (t, c) in tokens {
                format!("{:?}", t).hash(&mut hasher);
                c.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

/// CPN engine.
#[derive(Debug)]
pub struct CpnEngine {
    pub transitions: FxHashMap<TransitionId, Transition>,
    pub marking: Marking,
}

impl CpnEngine {
    pub fn new() -> Self {
        Self {
            transitions: FxHashMap::default(),
            marking: Marking::new(),
        }
    }

    pub fn add_transition(&mut self, t: Transition) {
        self.transitions.insert(t.id.clone(), t);
    }

    pub fn set_initial_marking(&mut self, marking: Marking) {
        self.marking = marking;
    }

    /// Resolve a token from binding or arc pattern.
    fn resolve_token(
        pattern: &ArcTokenPattern,
        binding: &FxHashMap<String, Token>,
    ) -> Option<Token> {
        match pattern {
            ArcTokenPattern::Concrete(t) => Some(t.clone()),
            ArcTokenPattern::Variable(v) => binding.get(v).cloned(),
        }
    }

    /// Fire a transition with the given binding.
    /// Returns Err(NotEnabled) if pre-conditions are not satisfied.
    pub fn fire(
        &mut self,
        transition_id: &str,
        binding: &FxHashMap<String, Token>,
    ) -> Result<(), NotEnabled> {
        let transition = self
            .transitions
            .get(transition_id)
            .ok_or_else(|| NotEnabled {
                transition: transition_id.to_string(),
                missing: vec![],
            })?;

        let mut missing = Vec::new();

        // Check pre-conditions (without consuming).
        for arc in &transition.pre {
            let token = Self::resolve_token(&arc.token, binding);
            let token = match token {
                Some(t) => t,
                None => {
                    missing.push((arc.place.clone(), Token::Unit));
                    continue;
                }
            };
            let place = self.marking.get(&arc.place);
            if place.map_or(true, |p| !p.contains(&token, 1)) {
                missing.push((arc.place.clone(), token));
            }
        }

        if !missing.is_empty() {
            return Err(NotEnabled {
                transition: transition_id.to_string(),
                missing,
            });
        }

        // Consume pre tokens.
        for arc in &transition.pre {
            let token = Self::resolve_token(&arc.token, binding).unwrap_or(Token::Unit);
            self.marking.get_or_insert(&arc.place).remove(&token, 1);
        }

        // Produce post tokens.
        for arc in &transition.post {
            let token = Self::resolve_token(&arc.token, binding);
            let token = token.unwrap_or(Token::Unit);
            self.marking.get_or_insert(&arc.place).add(token, 1);
        }

        Ok(())
    }

    pub fn marking(&self) -> &Marking {
        &self.marking
    }

    pub fn marking_mut(&mut self) -> &mut Marking {
        &mut self.marking
    }

    pub fn marking_hash(&self) -> u64 {
        self.marking.hash()
    }
}

impl Default for CpnEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Simple mutex model (Free(L) -> Held(L,tid)).
    #[test]
    fn test_mutex_acquire_release() {
        let mut cpn = CpnEngine::new();

        // Place "free" has one Lock(42) token.
        let mut init = Marking::new();
        init.add_token("free", Token::Lock(42), 1);

        // Transition acquire: pre free has Lock(L), post held has Lock(L).
        cpn.add_transition(Transition {
            id: "acquire".to_string(),
            pre: vec![ArcSpec {
                place: "free".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
            post: vec![ArcSpec {
                place: "held".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
        });

        // Transition release: pre held has Lock(L), post free has Lock(L).
        cpn.add_transition(Transition {
            id: "release".to_string(),
            pre: vec![ArcSpec {
                place: "held".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
            post: vec![ArcSpec {
                place: "free".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
        });

        cpn.set_initial_marking(init);

        let mut binding = FxHashMap::default();
        binding.insert("L".to_string(), Token::Lock(42));

        cpn.fire("acquire", &binding).unwrap();
        assert!(cpn.marking().get("free").unwrap().is_empty());
        assert_eq!(cpn.marking().get("held").unwrap().count(&Token::Lock(42)), 1);

        cpn.fire("release", &binding).unwrap();
        assert_eq!(cpn.marking().get("free").unwrap().count(&Token::Lock(42)), 1);
        assert!(cpn.marking().get("held").unwrap().is_empty());
    }

    /// Test 2: Two locks, concurrent firing.
    #[test]
    fn test_two_locks_concurrent() {
        let mut cpn = CpnEngine::new();

        let mut init = Marking::new();
        init.add_token("free", Token::Lock(1), 1);
        init.add_token("free", Token::Lock(2), 1);

        cpn.add_transition(Transition {
            id: "acquire".to_string(),
            pre: vec![ArcSpec {
                place: "free".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
            post: vec![ArcSpec {
                place: "held".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
        });

        cpn.set_initial_marking(init);

        let mut b1 = FxHashMap::default();
        b1.insert("L".to_string(), Token::Lock(1));
        let mut b2 = FxHashMap::default();
        b2.insert("L".to_string(), Token::Lock(2));

        cpn.fire("acquire", &b1).unwrap();
        cpn.fire("acquire", &b2).unwrap();

        assert!(cpn.marking().get("free").unwrap().is_empty());
        assert_eq!(cpn.marking().get("held").unwrap().count(&Token::Lock(1)), 1);
        assert_eq!(cpn.marking().get("held").unwrap().count(&Token::Lock(2)), 1);
    }

    /// Test 3: not-enabled returns readable error with missing place/token.
    #[test]
    fn test_not_enabled_error() {
        let mut cpn = CpnEngine::new();

        // Empty initial marking.
        cpn.set_initial_marking(Marking::new());

        cpn.add_transition(Transition {
            id: "acquire".to_string(),
            pre: vec![ArcSpec {
                place: "free".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
            post: vec![ArcSpec {
                place: "held".to_string(),
                token: ArcTokenPattern::Variable("L".to_string()),
            }],
        });

        let mut binding = FxHashMap::default();
        binding.insert("L".to_string(), Token::Lock(42));

        let err = cpn.fire("acquire", &binding).unwrap_err();
        assert_eq!(err.transition, "acquire");
        assert_eq!(err.missing.len(), 1);
        assert_eq!(err.missing[0].0, "free");
        assert_eq!(err.missing[0].1, Token::Lock(42));
    }
}
