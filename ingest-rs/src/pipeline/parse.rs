use crate::sources::github::*;
use crate::sources::stripe::*;
use crate::sources::custom::*;
use crate::sources::csv::*;

/// Trait for parsing raw JSON strings into typed event structs.
pub trait Parseable: Sized {
    fn parse(raw: &str) -> Result<Self, String>;
}

// --- GitHub event types ---

impl Parseable for PushEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("PushEvent parse: {}", e))
    }
}

impl Parseable for PullRequestEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("PullRequestEvent parse: {}", e))
    }
}

impl Parseable for IssueEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("IssueEvent parse: {}", e))
    }
}

impl Parseable for ReleaseEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("ReleaseEvent parse: {}", e))
    }
}

impl Parseable for DeploymentEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("DeploymentEvent parse: {}", e))
    }
}

impl Parseable for CheckRunEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("CheckRunEvent parse: {}", e))
    }
}

impl Parseable for WorkflowRunEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("WorkflowRunEvent parse: {}", e))
    }
}

// --- Stripe event types ---

impl Parseable for PaymentEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("PaymentEvent parse: {}", e))
    }
}

impl Parseable for InvoiceEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("InvoiceEvent parse: {}", e))
    }
}

impl Parseable for SubscriptionEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("SubscriptionEvent parse: {}", e))
    }
}

impl Parseable for RefundEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("RefundEvent parse: {}", e))
    }
}

impl Parseable for DisputeEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("DisputeEvent parse: {}", e))
    }
}

impl Parseable for ChargeEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("ChargeEvent parse: {}", e))
    }
}

// --- Custom event types ---

impl Parseable for CustomJsonEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("CustomJsonEvent parse: {}", e))
    }
}

impl Parseable for AlertEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("AlertEvent parse: {}", e))
    }
}

impl Parseable for MetricEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("MetricEvent parse: {}", e))
    }
}

impl Parseable for AuditEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("AuditEvent parse: {}", e))
    }
}

// --- CSV event types ---

impl Parseable for CsvTransactionRow {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("CsvTransactionRow parse: {}", e))
    }
}

impl Parseable for CsvInventoryRow {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("CsvInventoryRow parse: {}", e))
    }
}

impl Parseable for CsvUserActivityRow {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("CsvUserActivityRow parse: {}", e))
    }
}
