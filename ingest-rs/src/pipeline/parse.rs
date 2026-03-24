use crate::sources::github::*;
use crate::sources::stripe::*;
use crate::sources::custom::*;
use crate::sources::csv::*;
use crate::sources::cloud::*;
use crate::sources::observability::*;
use crate::sources::commerce::*;

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

// --- Cloud event types ---

impl Parseable for Ec2InstanceEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("Ec2InstanceEvent parse: {}", e))
    }
}

impl Parseable for S3BucketEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("S3BucketEvent parse: {}", e))
    }
}

impl Parseable for LambdaInvocationEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("LambdaInvocationEvent parse: {}", e))
    }
}

impl Parseable for CloudWatchAlarmEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("CloudWatchAlarmEvent parse: {}", e))
    }
}

impl Parseable for RdsEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("RdsEvent parse: {}", e))
    }
}

impl Parseable for EcsTaskEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("EcsTaskEvent parse: {}", e))
    }
}

impl Parseable for SqsMessageEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("SqsMessageEvent parse: {}", e))
    }
}

impl Parseable for SnsNotificationEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("SnsNotificationEvent parse: {}", e))
    }
}

// --- Observability event types ---

impl Parseable for LogEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("LogEvent parse: {}", e))
    }
}

impl Parseable for TraceSpanEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("TraceSpanEvent parse: {}", e))
    }
}

impl Parseable for MetricDatapointEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("MetricDatapointEvent parse: {}", e))
    }
}

impl Parseable for IncidentEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("IncidentEvent parse: {}", e))
    }
}

impl Parseable for PagerDutyAlertEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("PagerDutyAlertEvent parse: {}", e))
    }
}

impl Parseable for GrafanaAlertEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("GrafanaAlertEvent parse: {}", e))
    }
}

impl Parseable for DatadogEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("DatadogEvent parse: {}", e))
    }
}

impl Parseable for SentryErrorEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("SentryErrorEvent parse: {}", e))
    }
}

// --- Commerce event types ---

impl Parseable for OrderEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("OrderEvent parse: {}", e))
    }
}

impl Parseable for ShipmentEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("ShipmentEvent parse: {}", e))
    }
}

impl Parseable for InventoryChangeEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("InventoryChangeEvent parse: {}", e))
    }
}

impl Parseable for ReturnEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("ReturnEvent parse: {}", e))
    }
}

impl Parseable for ReviewEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("ReviewEvent parse: {}", e))
    }
}

impl Parseable for CouponEvent {
    fn parse(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("CouponEvent parse: {}", e))
    }
}
