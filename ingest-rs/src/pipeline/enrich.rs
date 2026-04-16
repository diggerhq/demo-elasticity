use chrono::Utc;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::unified::UnifiedEvent;

/// Enriches a unified event with computed metadata: dedup key, processing
/// timestamp, and additional tags. Generic over the original source type
/// to force monomorphization per source.
pub fn enrich<T: Serialize + DeserializeOwned>(mut event: UnifiedEvent) -> UnifiedEvent {
    // Update processing timestamp
    event.processed_at = Utc::now();

    // Generate a dedup key from source + event_type + resource_id + timestamp
    let dedup_input = format!(
        "{}:{}:{}:{}",
        event.source, event.event_type, event.resource_id, event.timestamp
    );
    let mut hasher = DefaultHasher::new();
    dedup_input.hash(&mut hasher);
    let dedup_key = format!("{:016x}", hasher.finish());

    // Attach dedup key to metadata
    if let serde_json::Value::Object(ref mut map) = event.metadata {
        map.insert("dedup_key".into(), serde_json::Value::String(dedup_key.clone()));
        map.insert(
            "enriched_at".into(),
            serde_json::Value::String(event.processed_at.to_rfc3339()),
        );

        // Compute payload size for observability
        let raw_size = serde_json::to_string(&event.raw_payload)
            .map(|s| s.len())
            .unwrap_or(0);
        map.insert(
            "raw_payload_bytes".into(),
            serde_json::Value::Number(serde_json::Number::from(raw_size)),
        );

        // Stamp the source type name from the generic parameter
        let type_name = std::any::type_name::<T>();
        map.insert(
            "source_type".into(),
            serde_json::Value::String(type_name.to_string()),
        );
    }

    // Ensure correlation_id is populated
    if event.correlation_id.is_empty() {
        event.correlation_id = dedup_key;
    }

    // Add enrichment tag
    if !event.tags.contains(&"enriched".to_string()) {
        event.tags.push("enriched".into());
    }

    event
}
