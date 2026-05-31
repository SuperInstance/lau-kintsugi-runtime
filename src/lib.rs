use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// BreakType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BreakType {
    TestFailure { test_name: String, error: String },
    BuildError { file: String, line: u32, error: String },
    RuntimePanic { message: String, backtrace: String },
    ConservationViolation { expected: f64, actual: f64 },
    Deadlock { thread: String, held_lock: String },
    Timeout { operation: String, limit_ms: u64 },
    ModelError { provider: String, code: u32, message: String },
    AgentCrash { agent_id: String, last_state: String },
    CircuitBreak { circuit_id: String, value: f64, threshold: f64 },
    Custom { category: String, message: String },
}

impl BreakType {
    pub fn category(&self) -> &str {
        match self {
            Self::TestFailure { .. } => "TestFailure",
            Self::BuildError { .. } => "BuildError",
            Self::RuntimePanic { .. } => "RuntimePanic",
            Self::ConservationViolation { .. } => "ConservationViolation",
            Self::Deadlock { .. } => "Deadlock",
            Self::Timeout { .. } => "Timeout",
            Self::ModelError { .. } => "ModelError",
            Self::AgentCrash { .. } => "AgentCrash",
            Self::CircuitBreak { .. } => "CircuitBreak",
            Self::Custom { category, .. } => category,
        }
    }
}

// ---------------------------------------------------------------------------
// KintsugiError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KintsugiError {
    ArtifactNotFound(String),
    AlreadyRepaired(String),
    NegativeValue(f64),
}

impl std::fmt::Display for KintsugiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArtifactNotFound(id) => write!(f, "artifact not found: {id}"),
            Self::AlreadyRepaired(id) => write!(f, "already repaired: {id}"),
            Self::NegativeValue(v) => write!(f, "negative value: {v}"),
        }
    }
}

impl std::error::Error for KintsugiError {}

// ---------------------------------------------------------------------------
// KintsugiRepair
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KintsugiRepair {
    pub id: String,
    pub artifact_id: String,
    pub break_type: BreakType,
    pub break_time: u64,
    pub repair_time: u64,
    pub golden_value: f64,
    pub repair_description: String,
    pub lessons: Vec<String>,
    pub repairer: String,
}

impl KintsugiRepair {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        artifact_id: impl Into<String>,
        break_type: BreakType,
        break_time: u64,
        repair_time: u64,
        golden_value: f64,
        repair_description: impl Into<String>,
        lessons: Vec<String>,
        repairer: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            artifact_id: artifact_id.into(),
            break_type,
            break_time,
            repair_time,
            golden_value,
            repair_description: repair_description.into(),
            lessons,
            repairer: repairer.into(),
        }
    }

    pub fn duration(&self) -> u64 {
        self.repair_time.saturating_sub(self.break_time)
    }

    pub fn value_density(&self) -> f64 {
        let dur = self.duration();
        if dur == 0 {
            self.golden_value
        } else {
            self.golden_value / dur as f64
        }
    }
}

// ---------------------------------------------------------------------------
// KintsugiArtifact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KintsugiArtifact {
    pub id: String,
    pub name: String,
    pub artifact_type: String,
    pub original_value: f64,
    pub break_count: u64,
    pub repair_count: u64,
    pub current_value: f64,
    pub repairs: Vec<KintsugiRepair>,
}

impl KintsugiArtifact {
    pub fn new(name: &str, artifact_type: &str) -> Self {
        let id = format!("artifact-{}-{}", name, uuid_stamp());
        Self {
            id,
            name: name.to_string(),
            artifact_type: artifact_type.to_string(),
            original_value: 1.0,
            break_count: 0,
            repair_count: 0,
            current_value: 1.0,
            repairs: Vec::new(),
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_original_value(mut self, v: f64) -> Self {
        self.original_value = v;
        self.current_value = v;
        self
    }

    pub fn break_and_repair(&mut self, repair: KintsugiRepair) {
        self.break_count += 1;
        self.repair_count += 1;
        self.current_value += repair.golden_value;
        self.repairs.push(repair);
    }

    pub fn total_golden_value(&self) -> f64 {
        self.repairs.iter().map(|r| r.golden_value).sum()
    }

    pub fn is_more_valuable(&self) -> bool {
        self.current_value > self.original_value
    }

    pub fn resilience_score(&self) -> f64 {
        if self.repairs.is_empty() {
            return 0.0;
        }
        let avg = self.total_golden_value() / self.repairs.len() as f64;
        (self.repair_count as f64).sqrt() * avg
    }

    pub fn weakest_break(&self) -> Option<&KintsugiRepair> {
        self.repairs.iter().max_by_key(|r| r.duration())
    }

    pub fn strongest_repair(&self) -> Option<&KintsugiRepair> {
        self.repairs.iter().max_by(|a, b| {
            a.golden_value
                .partial_cmp(&b.golden_value)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

fn uuid_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ---------------------------------------------------------------------------
// KintsugiPolicy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KintsugiPolicy {
    pub auto_repair: bool,
    pub golden_value_multiplier: f64,
    pub max_repairs_per_artifact: Option<usize>,
    pub decay_rate: f64,
    pub min_golden_value: f64,
}

impl Default for KintsugiPolicy {
    fn default() -> Self {
        Self {
            auto_repair: true,
            golden_value_multiplier: 1.1,
            max_repairs_per_artifact: None,
            decay_rate: 0.0,
            min_golden_value: 0.0,
        }
    }
}

impl KintsugiPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_auto_repair(mut self, v: bool) -> Self {
        self.auto_repair = v;
        self
    }

    pub fn with_multiplier(mut self, m: f64) -> Self {
        self.golden_value_multiplier = m;
        self
    }

    pub fn with_max_repairs(mut self, n: usize) -> Self {
        self.max_repairs_per_artifact = Some(n);
        self
    }

    pub fn with_decay_rate(mut self, r: f64) -> Self {
        self.decay_rate = r;
        self
    }

    pub fn with_min_golden_value(mut self, v: f64) -> Self {
        self.min_golden_value = v;
        self
    }

    pub fn is_within_limits(&self, artifact: &KintsugiArtifact) -> bool {
        match self.max_repairs_per_artifact {
            Some(max) => artifact.repair_count as usize <= max,
            None => true,
        }
    }
}

// ---------------------------------------------------------------------------
// KintsugiLedger
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KintsugiLedger {
    pub artifacts: HashMap<String, KintsugiArtifact>,
    pub total_breaks: u64,
    pub total_repairs: u64,
}

impl KintsugiLedger {
    pub fn new() -> Self {
        Self {
            artifacts: HashMap::new(),
            total_breaks: 0,
            total_repairs: 0,
        }
    }

    pub fn register_artifact(&mut self, artifact: KintsugiArtifact) {
        self.artifacts.insert(artifact.id.clone(), artifact);
    }

    pub fn record_repair(
        &mut self,
        artifact_id: &str,
        repair: KintsugiRepair,
    ) -> Result<(), KintsugiError> {
        let artifact = self
            .artifacts
            .get_mut(artifact_id)
            .ok_or_else(|| KintsugiError::ArtifactNotFound(artifact_id.to_string()))?;

        if repair.golden_value < 0.0 {
            return Err(KintsugiError::NegativeValue(repair.golden_value));
        }

        self.total_breaks += 1;
        self.total_repairs += 1;
        artifact.break_and_repair(repair);
        Ok(())
    }

    pub fn most_broken(&self, n: usize) -> Vec<&KintsugiArtifact> {
        let mut v: Vec<_> = self.artifacts.values().collect();
        v.sort_by_key(|b| std::cmp::Reverse(b.break_count));
        v.into_iter().take(n).collect()
    }

    pub fn most_valuable(&self, n: usize) -> Vec<&KintsugiArtifact> {
        let mut v: Vec<_> = self.artifacts.values().collect();
        v.sort_by(|a, b| {
            b.current_value
                .partial_cmp(&a.current_value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v.into_iter().take(n).collect()
    }

    pub fn most_resilient(&self, n: usize) -> Vec<&KintsugiArtifact> {
        let mut v: Vec<_> = self.artifacts.values().collect();
        v.sort_by(|a, b| {
            b.resilience_score()
                .partial_cmp(&a.resilience_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v.into_iter().take(n).collect()
    }

    pub fn total_golden_value(&self) -> f64 {
        self.artifacts.values().map(|a| a.total_golden_value()).sum()
    }

    pub fn break_type_distribution(&self) -> HashMap<String, u64> {
        let mut dist: HashMap<String, u64> = HashMap::new();
        for artifact in self.artifacts.values() {
            for repair in &artifact.repairs {
                *dist.entry(repair.break_type.category().to_string()).or_insert(0) += 1;
            }
        }
        dist
    }

    pub fn lessons_learned(&self) -> Vec<&str> {
        self.artifacts
            .values()
            .flat_map(|a| a.repairs.iter().flat_map(|r| r.lessons.iter().map(|s| s.as_str())))
            .collect()
    }
}

impl Default for KintsugiLedger {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// lib.rs re-exports
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_break() -> BreakType {
        BreakType::TestFailure {
            test_name: "test_foo".into(),
            error: "assertion failed".into(),
        }
    }

    fn make_repair(artifact_id: &str, golden_value: f64) -> KintsugiRepair {
        KintsugiRepair::new(
            format!("repair-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
            artifact_id,
            make_break(),
            1000,
            2000,
            golden_value,
            "fixed the thing",
            vec!["always check nulls".into()],
            "agent-1",
        )
    }

    // --- BreakType tests ---

    #[test]
    fn break_type_categories() {
        assert_eq!(BreakType::TestFailure { test_name: "t".into(), error: "e".into() }.category(), "TestFailure");
        assert_eq!(BreakType::BuildError { file: "f".into(), line: 1, error: "e".into() }.category(), "BuildError");
        assert_eq!(BreakType::RuntimePanic { message: "m".into(), backtrace: "bt".into() }.category(), "RuntimePanic");
        assert_eq!(BreakType::ConservationViolation { expected: 1.0, actual: 0.5 }.category(), "ConservationViolation");
        assert_eq!(BreakType::Deadlock { thread: "t".into(), held_lock: "l".into() }.category(), "Deadlock");
        assert_eq!(BreakType::Timeout { operation: "op".into(), limit_ms: 100 }.category(), "Timeout");
        assert_eq!(BreakType::ModelError { provider: "p".into(), code: 429, message: "m".into() }.category(), "ModelError");
        assert_eq!(BreakType::AgentCrash { agent_id: "a".into(), last_state: "s".into() }.category(), "AgentCrash");
        assert_eq!(BreakType::CircuitBreak { circuit_id: "c".into(), value: 1.0, threshold: 0.5 }.category(), "CircuitBreak");
        assert_eq!(BreakType::Custom { category: "Weird".into(), message: "m".into() }.category(), "Weird");
    }

    #[test]
    fn break_type_serde_roundtrip() {
        let bt = BreakType::Deadlock { thread: "t1".into(), held_lock: "mutex".into() };
        let json = serde_json::to_string(&bt).unwrap();
        let bt2: BreakType = serde_json::from_str(&json).unwrap();
        assert_eq!(bt, bt2);
    }

    // --- KintsugiRepair tests ---

    #[test]
    fn repair_duration() {
        let r = make_repair("a1", 1.0);
        assert_eq!(r.duration(), 1000);
    }

    #[test]
    fn repair_duration_zero() {
        let r = KintsugiRepair::new("r1", "a1", make_break(), 1000, 1000, 1.0, "desc", vec![], "agent");
        assert_eq!(r.duration(), 0);
    }

    #[test]
    fn repair_duration_wrap_around() {
        let r = KintsugiRepair::new("r1", "a1", make_break(), 2000, 1000, 1.0, "desc", vec![], "agent");
        assert_eq!(r.duration(), 0); // saturating_sub
    }

    #[test]
    fn repair_value_density() {
        let r = make_repair("a1", 10.0);
        assert!((r.value_density() - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn repair_value_density_zero_duration() {
        let r = KintsugiRepair::new("r1", "a1", make_break(), 1000, 1000, 5.0, "d", vec![], "a");
        assert!((r.value_density() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn repair_serde_roundtrip() {
        let r = make_repair("a1", 2.5);
        let json = serde_json::to_string(&r).unwrap();
        let r2: KintsugiRepair = serde_json::from_str(&json).unwrap();
        assert_eq!(r2.artifact_id, "a1");
        assert!((r2.golden_value - 2.5).abs() < f64::EPSILON);
    }

    // --- KintsugiArtifact tests ---

    #[test]
    fn artifact_new_defaults() {
        let a = KintsugiArtifact::new("my-service", "microservice");
        assert_eq!(a.name, "my-service");
        assert_eq!(a.artifact_type, "microservice");
        assert!((a.original_value - 1.0).abs() < f64::EPSILON);
        assert_eq!(a.break_count, 0);
        assert_eq!(a.repair_count, 0);
    }

    #[test]
    fn artifact_break_and_repair() {
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        let r = make_repair("a1", 0.5);
        a.break_and_repair(r);
        assert_eq!(a.break_count, 1);
        assert_eq!(a.repair_count, 1);
        assert!((a.current_value - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn artifact_multiple_repairs() {
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1").with_original_value(10.0);
        a.break_and_repair(make_repair("a1", 2.0));
        a.break_and_repair(make_repair("a1", 3.0));
        assert_eq!(a.break_count, 2);
        assert!((a.current_value - 15.0).abs() < f64::EPSILON);
        assert!((a.total_golden_value() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn artifact_is_more_valuable() {
        let mut a = KintsugiArtifact::new("svc", "service").with_original_value(10.0);
        assert!(!a.is_more_valuable());
        a.break_and_repair(make_repair("a1", 5.0));
        assert!(a.is_more_valuable());
    }

    #[test]
    fn artifact_not_more_valuable_with_zero() {
        let mut a = KintsugiArtifact::new("svc", "service").with_original_value(10.0);
        a.current_value = 10.0;
        assert!(!a.is_more_valuable());
    }

    #[test]
    fn artifact_resilience_score() {
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        a.break_and_repair(make_repair("a1", 4.0));
        a.break_and_repair(make_repair("a1", 6.0));
        // avg golden = 5.0, sqrt(2) * 5.0 ≈ 7.07
        let score = a.resilience_score();
        assert!((score - 5.0 * 2.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn artifact_resilience_score_empty() {
        let a = KintsugiArtifact::new("svc", "service");
        assert!((a.resilience_score()).abs() < f64::EPSILON);
    }

    #[test]
    fn artifact_weakest_break() {
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        let r1 = KintsugiRepair::new("r1", "a1", make_break(), 100, 200, 1.0, "d", vec![], "a");
        let r2 = KintsugiRepair::new("r2", "a1", make_break(), 100, 500, 1.0, "d", vec![], "a");
        a.break_and_repair(r1);
        a.break_and_repair(r2);
        let weakest = a.weakest_break().unwrap();
        assert_eq!(weakest.id, "r2");
    }

    #[test]
    fn artifact_weakest_break_empty() {
        let a = KintsugiArtifact::new("svc", "service");
        assert!(a.weakest_break().is_none());
    }

    #[test]
    fn artifact_strongest_repair() {
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        let r1 = KintsugiRepair::new("r1", "a1", make_break(), 100, 200, 1.0, "d", vec![], "a");
        let r2 = KintsugiRepair::new("r2", "a1", make_break(), 100, 200, 5.0, "d", vec![], "a");
        a.break_and_repair(r1);
        a.break_and_repair(r2);
        let strongest = a.strongest_repair().unwrap();
        assert_eq!(strongest.id, "r2");
    }

    #[test]
    fn artifact_strongest_repair_empty() {
        let a = KintsugiArtifact::new("svc", "service");
        assert!(a.strongest_repair().is_none());
    }

    #[test]
    fn artifact_serde_roundtrip() {
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        a.break_and_repair(make_repair("a1", 3.0));
        let json = serde_json::to_string(&a).unwrap();
        let a2: KintsugiArtifact = serde_json::from_str(&json).unwrap();
        assert_eq!(a2.id, "a1");
        assert_eq!(a2.repair_count, 1);
    }

    #[test]
    fn artifact_with_original_value() {
        let a = KintsugiArtifact::new("svc", "service").with_original_value(42.0);
        assert!((a.original_value - 42.0).abs() < f64::EPSILON);
        assert!((a.current_value - 42.0).abs() < f64::EPSILON);
    }

    // --- KintsugiPolicy tests ---

    #[test]
    fn policy_defaults() {
        let p = KintsugiPolicy::default();
        assert!(p.auto_repair);
        assert!((p.golden_value_multiplier - 1.1).abs() < f64::EPSILON);
        assert!(p.max_repairs_per_artifact.is_none());
        assert!((p.decay_rate).abs() < f64::EPSILON);
        assert!((p.min_golden_value).abs() < f64::EPSILON);
    }

    #[test]
    fn policy_builder() {
        let p = KintsugiPolicy::new()
            .with_auto_repair(false)
            .with_multiplier(2.0)
            .with_max_repairs(5)
            .with_decay_rate(0.1)
            .with_min_golden_value(0.5);
        assert!(!p.auto_repair);
        assert!((p.golden_value_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(p.max_repairs_per_artifact, Some(5));
        assert!((p.decay_rate - 0.1).abs() < f64::EPSILON);
        assert!((p.min_golden_value - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn policy_within_limits() {
        let p = KintsugiPolicy::new().with_max_repairs(2);
        let a = KintsugiArtifact::new("svc", "service");
        assert!(p.is_within_limits(&a));
    }

    #[test]
    fn policy_exceeds_limits() {
        let p = KintsugiPolicy::new().with_max_repairs(1);
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        a.break_and_repair(make_repair("a1", 1.0));
        a.break_and_repair(make_repair("a1", 1.0));
        assert!(!p.is_within_limits(&a));
    }

    #[test]
    fn policy_no_limit() {
        let p = KintsugiPolicy::new(); // no max
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        for _ in 0..100 {
            a.break_and_repair(make_repair("a1", 1.0));
        }
        assert!(p.is_within_limits(&a));
    }

    #[test]
    fn policy_serde_roundtrip() {
        let p = KintsugiPolicy::new().with_multiplier(3.0).with_max_repairs(10);
        let json = serde_json::to_string(&p).unwrap();
        let p2: KintsugiPolicy = serde_json::from_str(&json).unwrap();
        assert!((p2.golden_value_multiplier - 3.0).abs() < f64::EPSILON);
        assert_eq!(p2.max_repairs_per_artifact, Some(10));
    }

    // --- KintsugiError tests ---

    #[test]
    fn error_display() {
        assert_eq!(
            KintsugiError::ArtifactNotFound("x".into()).to_string(),
            "artifact not found: x"
        );
        assert_eq!(
            KintsugiError::AlreadyRepaired("r1".into()).to_string(),
            "already repaired: r1"
        );
        assert_eq!(
            KintsugiError::NegativeValue(-1.5).to_string(),
            "negative value: -1.5"
        );
    }

    #[test]
    fn error_equality() {
        assert_eq!(
            KintsugiError::ArtifactNotFound("a".into()),
            KintsugiError::ArtifactNotFound("a".into())
        );
        assert_ne!(
            KintsugiError::ArtifactNotFound("a".into()),
            KintsugiError::ArtifactNotFound("b".into())
        );
    }

    #[test]
    fn error_serde_roundtrip() {
        let e = KintsugiError::NegativeValue(-3.14);
        let json = serde_json::to_string(&e).unwrap();
        let e2: KintsugiError = serde_json::from_str(&json).unwrap();
        assert_eq!(e, e2);
    }

    // --- KintsugiLedger tests ---

    #[test]
    fn ledger_new_is_empty() {
        let l = KintsugiLedger::new();
        assert!(l.artifacts.is_empty());
        assert_eq!(l.total_breaks, 0);
        assert_eq!(l.total_repairs, 0);
    }

    #[test]
    fn ledger_register_artifact() {
        let mut l = KintsugiLedger::new();
        let a = KintsugiArtifact::new("svc", "service").with_id("a1");
        l.register_artifact(a);
        assert!(l.artifacts.contains_key("a1"));
    }

    #[test]
    fn ledger_record_repair() {
        let mut l = KintsugiLedger::new();
        let a = KintsugiArtifact::new("svc", "service").with_id("a1");
        l.register_artifact(a);
        let r = make_repair("a1", 2.0);
        l.record_repair("a1", r).unwrap();
        assert_eq!(l.total_breaks, 1);
        assert_eq!(l.total_repairs, 1);
        assert!((l.artifacts["a1"].current_value - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ledger_record_repair_not_found() {
        let mut l = KintsugiLedger::new();
        let r = make_repair("ghost", 1.0);
        let err = l.record_repair("ghost", r);
        assert!(matches!(err, Err(KintsugiError::ArtifactNotFound(_))));
    }

    #[test]
    fn ledger_record_repair_negative_value() {
        let mut l = KintsugiLedger::new();
        l.register_artifact(KintsugiArtifact::new("svc", "service").with_id("a1"));
        let r = KintsugiRepair::new("r1", "a1", make_break(), 100, 200, -1.0, "d", vec![], "a");
        let err = l.record_repair("a1", r);
        assert!(matches!(err, Err(KintsugiError::NegativeValue(_))));
    }

    #[test]
    fn ledger_most_broken() {
        let mut l = KintsugiLedger::new();
        let mut a1 = KintsugiArtifact::new("a", "service").with_id("a1");
        let mut a2 = KintsugiArtifact::new("b", "service").with_id("a2");
        for _ in 0..5 { a1.break_and_repair(make_repair("a1", 1.0)); }
        for _ in 0..2 { a2.break_and_repair(make_repair("a2", 1.0)); }
        l.register_artifact(a1);
        l.register_artifact(a2);
        let top = l.most_broken(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].id, "a1");
    }

    #[test]
    fn ledger_most_valuable() {
        let mut l = KintsugiLedger::new();
        let mut a1 = KintsugiArtifact::new("a", "service").with_id("a1").with_original_value(5.0);
        a1.current_value = 100.0;
        let mut a2 = KintsugiArtifact::new("b", "service").with_id("a2").with_original_value(5.0);
        a2.current_value = 50.0;
        l.register_artifact(a1);
        l.register_artifact(a2);
        let top = l.most_valuable(1);
        assert_eq!(top[0].id, "a1");
    }

    #[test]
    fn ledger_most_resilient() {
        let mut l = KintsugiLedger::new();
        let mut a1 = KintsugiArtifact::new("a", "service").with_id("a1");
        let mut a2 = KintsugiArtifact::new("b", "service").with_id("a2");
        for _ in 0..3 { a1.break_and_repair(make_repair("a1", 2.0)); }
        a2.break_and_repair(make_repair("a2", 0.5));
        l.register_artifact(a1);
        l.register_artifact(a2);
        let top = l.most_resilient(1);
        assert_eq!(top[0].id, "a1");
    }

    #[test]
    fn ledger_total_golden_value() {
        let mut l = KintsugiLedger::new();
        let mut a1 = KintsugiArtifact::new("a", "service").with_id("a1");
        let mut a2 = KintsugiArtifact::new("b", "service").with_id("a2");
        a1.break_and_repair(make_repair("a1", 3.0));
        a2.break_and_repair(make_repair("a2", 7.0));
        l.register_artifact(a1);
        l.register_artifact(a2);
        assert!((l.total_golden_value() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ledger_break_type_distribution() {
        let mut l = KintsugiLedger::new();
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        let r1 = KintsugiRepair::new("r1", "a1", BreakType::TestFailure { test_name: "t".into(), error: "e".into() }, 100, 200, 1.0, "d", vec![], "a");
        let r2 = KintsugiRepair::new("r2", "a1", BreakType::Timeout { operation: "op".into(), limit_ms: 100 }, 100, 200, 1.0, "d", vec![], "a");
        let r3 = KintsugiRepair::new("r3", "a1", BreakType::TestFailure { test_name: "t2".into(), error: "e2".into() }, 100, 200, 1.0, "d", vec![], "a");
        a.break_and_repair(r1);
        a.break_and_repair(r2);
        a.break_and_repair(r3);
        l.register_artifact(a);
        let dist = l.break_type_distribution();
        assert_eq!(*dist.get("TestFailure").unwrap(), 2);
        assert_eq!(*dist.get("Timeout").unwrap(), 1);
    }

    #[test]
    fn ledger_lessons_learned() {
        let mut l = KintsugiLedger::new();
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        let r1 = KintsugiRepair::new("r1", "a1", make_break(), 100, 200, 1.0, "d", vec!["lesson 1".into()], "a");
        let r2 = KintsugiRepair::new("r2", "a1", make_break(), 100, 200, 1.0, "d", vec!["lesson 2".into(), "lesson 3".into()], "a");
        a.break_and_repair(r1);
        a.break_and_repair(r2);
        l.register_artifact(a);
        let lessons = l.lessons_learned();
        assert_eq!(lessons, vec!["lesson 1", "lesson 2", "lesson 3"]);
    }

    #[test]
    fn ledger_most_broken_n_exceeds_count() {
        let mut l = KintsugiLedger::new();
        l.register_artifact(KintsugiArtifact::new("a", "s").with_id("a1"));
        let top = l.most_broken(10);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn ledger_serde_roundtrip() {
        let mut l = KintsugiLedger::new();
        l.register_artifact(KintsugiArtifact::new("svc", "service").with_id("a1"));
        l.record_repair("a1", make_repair("a1", 4.0)).unwrap();
        let json = serde_json::to_string(&l).unwrap();
        let l2: KintsugiLedger = serde_json::from_str(&json).unwrap();
        assert_eq!(l2.total_breaks, 1);
        assert_eq!(l2.total_repairs, 1);
        assert!(l2.artifacts.contains_key("a1"));
        assert!((l2.artifacts["a1"].current_value - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ledger_default() {
        let l = KintsugiLedger::default();
        assert!(l.artifacts.is_empty());
    }

    // --- Integration / end-to-end style tests ---

    #[test]
    fn kintsugi_journey() {
        let mut ledger = KintsugiLedger::new();
        let artifact = KintsugiArtifact::new("auth-service", "microservice")
            .with_id("auth")
            .with_original_value(100.0);
        ledger.register_artifact(artifact);

        // First break: timeout
        let r1 = KintsugiRepair::new(
            "r1", "auth",
            BreakType::Timeout { operation: "db_query".into(), limit_ms: 5000 },
            1000, 1200,
            15.0,
            "added connection pooling",
            vec!["always pool connections".into()],
            "agent-resilience",
        );
        ledger.record_repair("auth", r1).unwrap();

        // Second break: test failure
        let r2 = KintsugiRepair::new(
            "r2", "auth",
            BreakType::TestFailure { test_name: "test_login".into(), error: "race condition".into() },
            2000, 2500,
            25.0,
            "added mutex around login state",
            vec!["shared state needs synchronization".into()],
            "agent-resilience",
        );
        ledger.record_repair("auth", r2).unwrap();

        let a = &ledger.artifacts["auth"];
        assert_eq!(a.break_count, 2);
        assert!(a.is_more_valuable());
        assert!((a.current_value - 140.0).abs() < f64::EPSILON);
        assert!((a.total_golden_value() - 40.0).abs() < f64::EPSILON);
        assert_eq!(ledger.total_golden_value(), a.total_golden_value());
    }

    #[test]
    fn all_break_types_in_ledger() {
        let mut l = KintsugiLedger::new();
        let mut a = KintsugiArtifact::new("mega", "system").with_id("mega").with_original_value(0.0);
        let breaks = vec![
            BreakType::TestFailure { test_name: "t".into(), error: "e".into() },
            BreakType::BuildError { file: "f.rs".into(), line: 42, error: "e".into() },
            BreakType::RuntimePanic { message: "oom".into(), backtrace: "bt".into() },
            BreakType::ConservationViolation { expected: 1.0, actual: 0.5 },
            BreakType::Deadlock { thread: "t".into(), held_lock: "m".into() },
            BreakType::Timeout { operation: "op".into(), limit_ms: 100 },
            BreakType::ModelError { provider: "openai".into(), code: 429, message: "rate limited".into() },
            BreakType::AgentCrash { agent_id: "a1".into(), last_state: "running".into() },
            BreakType::CircuitBreak { circuit_id: "c1".into(), value: 10.0, threshold: 5.0 },
            BreakType::Custom { category: "Cosmic".into(), message: "solar flare".into() },
        ];
        for (i, bt) in breaks.into_iter().enumerate() {
            let r = KintsugiRepair::new(
                format!("r{i}"), "mega", bt, 100, 200, 1.0, "d", vec![], "a",
            );
            a.break_and_repair(r);
        }
        l.register_artifact(a);
        let dist = l.break_type_distribution();
        assert_eq!(dist.len(), 10); // all different categories
        assert_eq!(dist.values().sum::<u64>(), 10);
    }

    #[test]
    fn resilience_score_single_repair() {
        let mut a = KintsugiArtifact::new("svc", "service").with_id("a1");
        a.break_and_repair(make_repair("a1", 9.0));
        // sqrt(1) * 9.0 = 9.0
        assert!((a.resilience_score() - 9.0).abs() < 1e-9);
    }

    #[test]
    fn ledger_lessons_empty() {
        let mut l = KintsugiLedger::new();
        l.register_artifact(KintsugiArtifact::new("svc", "s").with_id("a1"));
        assert!(l.lessons_learned().is_empty());
    }

    #[test]
    fn ledger_break_distribution_empty() {
        let l = KintsugiLedger::new();
        assert!(l.break_type_distribution().is_empty());
    }

    #[test]
    fn repair_new_constructor() {
        let r = KintsugiRepair::new("r1", "a1", make_break(), 100, 300, 5.0, "fixed", vec!["l1".into()], "bot");
        assert_eq!(r.id, "r1");
        assert_eq!(r.artifact_id, "a1");
        assert_eq!(r.break_time, 100);
        assert_eq!(r.repair_time, 300);
        assert!((r.golden_value - 5.0).abs() < f64::EPSILON);
        assert_eq!(r.repair_description, "fixed");
        assert_eq!(r.lessons, vec!["l1"]);
        assert_eq!(r.repairer, "bot");
    }

    #[test]
    fn artifact_builder_chain() {
        let a = KintsugiArtifact::new("x", "y").with_id("custom-id").with_original_value(99.0);
        assert_eq!(a.id, "custom-id");
        assert!((a.original_value - 99.0).abs() < f64::EPSILON);
        assert!((a.current_value - 99.0).abs() < f64::EPSILON);
    }

    #[test]
    fn artifact_total_golden_value_empty() {
        let a = KintsugiArtifact::new("s", "t");
        assert!((a.total_golden_value()).abs() < f64::EPSILON);
    }

    #[test]
    fn break_type_equality() {
        let a = BreakType::Timeout { operation: "op".into(), limit_ms: 100 };
        let b = BreakType::Timeout { operation: "op".into(), limit_ms: 100 };
        assert_eq!(a, b);
    }

    #[test]
    fn break_type_inequality() {
        let a = BreakType::Timeout { operation: "op1".into(), limit_ms: 100 };
        let b = BreakType::Timeout { operation: "op2".into(), limit_ms: 100 };
        assert_ne!(a, b);
    }

    #[test]
    fn error_is_std_error() {
        let e = KintsugiError::ArtifactNotFound("x".into());
        let _: &dyn std::error::Error = &e;
    }

    #[test]
    fn policy_new_equals_default() {
        let n = KintsugiPolicy::new();
        let d = KintsugiPolicy::default();
        assert_eq!(n.auto_repair, d.auto_repair);
        assert!((n.golden_value_multiplier - d.golden_value_multiplier).abs() < f64::EPSILON);
        assert_eq!(n.max_repairs_per_artifact, d.max_repairs_per_artifact);
    }

    #[test]
    fn ledger_record_multiple_repairs_same_artifact() {
        let mut l = KintsugiLedger::new();
        l.register_artifact(KintsugiArtifact::new("s", "svc").with_id("a1"));
        for _ in 0..5 {
            l.record_repair("a1", make_repair("a1", 1.0)).unwrap();
        }
        assert_eq!(l.total_breaks, 5);
        assert_eq!(l.total_repairs, 5);
        let a = &l.artifacts["a1"];
        assert_eq!(a.repair_count, 5);
        assert!((a.current_value - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn artifact_weakest_break_same_duration() {
        let mut a = KintsugiArtifact::new("s", "t").with_id("a1");
        let r1 = KintsugiRepair::new("r1", "a1", make_break(), 100, 200, 1.0, "d", vec![], "a");
        let r2 = KintsugiRepair::new("r2", "a1", make_break(), 100, 200, 2.0, "d", vec![], "a");
        a.break_and_repair(r1);
        a.break_and_repair(r2);
        // Both have duration 100, so max_by_key picks one of them
        let w = a.weakest_break().unwrap();
        assert!(w.id == "r1" || w.id == "r2");
    }

    #[test]
    fn ledger_golden_value_empty() {
        let l = KintsugiLedger::new();
        assert!((l.total_golden_value()).abs() < f64::EPSILON);
    }

    #[test]
    fn ledger_multiple_artifacts_lessons() {
        let mut l = KintsugiLedger::new();
        let mut a1 = KintsugiArtifact::new("a", "svc").with_id("a1");
        let mut a2 = KintsugiArtifact::new("b", "svc").with_id("a2");
        let r1 = KintsugiRepair::new("r1", "a1", make_break(), 100, 200, 1.0, "d", vec!["L1".into()], "a");
        let r2 = KintsugiRepair::new("r2", "a2", make_break(), 100, 200, 1.0, "d", vec!["L2".into()], "a");
        a1.break_and_repair(r1);
        a2.break_and_repair(r2);
        l.register_artifact(a1);
        l.register_artifact(a2);
        let mut lessons: Vec<_> = l.lessons_learned().into_iter().collect();
        lessons.sort();
        assert_eq!(lessons, vec!["L1", "L2"]);
    }

    #[test]
    fn multiple_artifacts_ranking() {
        let mut l = KintsugiLedger::new();
        let ids = ["alpha", "beta", "gamma"];
        let values = [10.0, 50.0, 30.0];
        for (id, val) in ids.iter().zip(values.iter()) {
            let mut a = KintsugiArtifact::new(*id, "service").with_id(*id).with_original_value(0.0);
            a.break_and_repair(make_repair(id, *val));
            l.register_artifact(a);
        }
        let top = l.most_valuable(2);
        assert_eq!(top[0].id, "beta");
        assert_eq!(top[1].id, "gamma");
    }
}
