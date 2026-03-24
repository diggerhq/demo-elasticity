//! Storage backend for persisting processed events to S3.
//! Uses aws-sdk-s3 + reqwest for uploading batches.

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use reqwest::Client as HttpClient;
use serde::Serialize;

use crate::unified::UnifiedEvent;

/// S3-backed storage for processed events.
pub struct EventStore {
    s3: S3Client,
    http: HttpClient,
    bucket: String,
}

impl EventStore {
    pub async fn new(bucket: &str) -> Self {
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        Self {
            s3: S3Client::new(&config),
            http: HttpClient::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("http client"),
            bucket: bucket.to_string(),
        }
    }

    /// Upload a batch of events as a JSON Lines file to S3.
    pub async fn upload_batch(&self, key: &str, events: &[UnifiedEvent]) -> Result<(), String> {
        let body: String = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");

        self.s3
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body.into_bytes().into())
            .send()
            .await
            .map_err(|e| format!("S3 upload failed: {}", e))?;

        Ok(())
    }

    /// Webhook notification via HTTP POST after batch upload.
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
