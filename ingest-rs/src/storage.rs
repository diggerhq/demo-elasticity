//! Storage backend for persisting processed events via HTTP.
//! Uses reqwest for uploading batches to a remote endpoint.

use reqwest::Client as HttpClient;
use serde::Serialize;

use crate::unified::UnifiedEvent;

/// HTTP-backed storage for processed events.
pub struct EventStore {
    http: HttpClient,
    endpoint: String,
}

impl EventStore {
    pub fn new(endpoint: &str) -> Self {
        Self {
            http: HttpClient::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("http client"),
            endpoint: endpoint.to_string(),
        }
    }

    /// Upload a batch of events as JSON to the configured endpoint.
    pub async fn upload_batch(&self, events: &[UnifiedEvent]) -> Result<(), String> {
        self.http
            .post(&self.endpoint)
            .json(&events)
            .send()
            .await
            .map_err(|e| format!("upload failed: {}", e))?;
        Ok(())
    }

    /// Webhook notification via HTTP POST.
    pub async fn notify_webhook(&self, url: &str, event: &impl Serialize) -> Result<(), String> {
        self.http
            .post(url)
            .json(event)
            .send()
            .await
            .map_err(|e| format!("webhook notify failed: {}", e))?;
        Ok(())
    }
}
