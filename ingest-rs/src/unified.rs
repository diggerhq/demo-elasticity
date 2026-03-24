use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unified event type — the canonical output of the normalization pipeline.
/// Every source event (GitHub, Stripe, CSV, custom) gets mapped to this format
/// before enrichment and emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedEvent {
    pub id: String,
    pub source: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    pub severity: String,
    pub correlation_id: String,
    pub processed_at: DateTime<Utc>,
    pub raw_payload: serde_json::Value,
    pub version: String,
}
