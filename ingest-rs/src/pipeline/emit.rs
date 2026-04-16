use serde::{de::DeserializeOwned, Serialize};

use crate::unified::UnifiedEvent;

/// Serialize a slice of unified events to JSON Lines format.
/// Generic over the original source type to force monomorphization.
pub fn emit<T: Serialize + DeserializeOwned>(events: &[UnifiedEvent]) -> Result<String, String> {
    let mut lines = Vec::with_capacity(events.len());
    for event in events {
        let line = serde_json::to_string(event)
            .map_err(|e| format!("emit serialization error: {}", e))?;
        lines.push(line);
    }
    Ok(lines.join("\n"))
}
