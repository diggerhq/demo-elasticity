use crate::sources::github::*;
use crate::sources::stripe::*;
use crate::sources::custom::*;
use crate::sources::csv::*;

/// Trait for validating parsed event structs against business rules.
pub trait Validatable {
    fn validate(&self) -> Result<(), String>;
}

// --- GitHub event types ---

impl Validatable for PushEvent {
    fn validate(&self) -> Result<(), String> {
        if self.ref_name.is_empty() {
            return Err("PushEvent: ref_name is required".into());
        }
        if self.after.is_empty() {
            return Err("PushEvent: after commit SHA is required".into());
        }
        if self.repository.is_empty() {
            return Err("PushEvent: repository is required".into());
        }
        Ok(())
    }
}

impl Validatable for PullRequestEvent {
    fn validate(&self) -> Result<(), String> {
        if self.number <= 0 {
            return Err("PullRequestEvent: number must be positive".into());
        }
        if self.title.is_empty() {
            return Err("PullRequestEvent: title is required".into());
        }
        if self.action.is_empty() {
            return Err("PullRequestEvent: action is required".into());
        }
        if self.head_ref.is_empty() || self.base_ref.is_empty() {
            return Err("PullRequestEvent: head_ref and base_ref are required".into());
        }
        if self.additions < 0 || self.deletions < 0 {
            return Err("PullRequestEvent: additions/deletions cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for IssueEvent {
    fn validate(&self) -> Result<(), String> {
        if self.number <= 0 {
            return Err("IssueEvent: number must be positive".into());
        }
        if self.title.is_empty() {
            return Err("IssueEvent: title is required".into());
        }
        if self.action.is_empty() {
            return Err("IssueEvent: action is required".into());
        }
        if !["open", "closed"].contains(&self.state.as_str()) {
            return Err("IssueEvent: state must be 'open' or 'closed'".into());
        }
        Ok(())
    }
}

impl Validatable for ReleaseEvent {
    fn validate(&self) -> Result<(), String> {
        if self.tag_name.is_empty() {
            return Err("ReleaseEvent: tag_name is required".into());
        }
        if self.action.is_empty() {
            return Err("ReleaseEvent: action is required".into());
        }
        if self.author.is_empty() {
            return Err("ReleaseEvent: author is required".into());
        }
        Ok(())
    }
}

impl Validatable for DeploymentEvent {
    fn validate(&self) -> Result<(), String> {
        if self.environment.is_empty() {
            return Err("DeploymentEvent: environment is required".into());
        }
        if self.sha.len() != 40 {
            return Err("DeploymentEvent: sha must be 40 characters".into());
        }
        if self.ref_name.is_empty() {
            return Err("DeploymentEvent: ref_name is required".into());
        }
        Ok(())
    }
}

impl Validatable for CheckRunEvent {
    fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("CheckRunEvent: name is required".into());
        }
        if self.head_sha.is_empty() {
            return Err("CheckRunEvent: head_sha is required".into());
        }
        if !["queued", "in_progress", "completed"].contains(&self.status.as_str()) {
            return Err("CheckRunEvent: invalid status".into());
        }
        Ok(())
    }
}

impl Validatable for WorkflowRunEvent {
    fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("WorkflowRunEvent: name is required".into());
        }
        if self.head_sha.is_empty() {
            return Err("WorkflowRunEvent: head_sha is required".into());
        }
        if self.run_number <= 0 {
            return Err("WorkflowRunEvent: run_number must be positive".into());
        }
        if self.run_attempt <= 0 {
            return Err("WorkflowRunEvent: run_attempt must be positive".into());
        }
        Ok(())
    }
}

// --- Stripe event types ---

impl Validatable for PaymentEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("PaymentEvent: id is required".into());
        }
        if self.amount < 0 {
            return Err("PaymentEvent: amount cannot be negative".into());
        }
        if self.currency.len() != 3 {
            return Err("PaymentEvent: currency must be 3 characters".into());
        }
        if self.payment_method.is_empty() {
            return Err("PaymentEvent: payment_method is required".into());
        }
        Ok(())
    }
}

impl Validatable for InvoiceEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("InvoiceEvent: id is required".into());
        }
        if self.customer.is_empty() {
            return Err("InvoiceEvent: customer is required".into());
        }
        if self.currency.len() != 3 {
            return Err("InvoiceEvent: currency must be 3 characters".into());
        }
        if self.amount_due < 0 {
            return Err("InvoiceEvent: amount_due cannot be negative".into());
        }
        if self.attempt_count < 0 {
            return Err("InvoiceEvent: attempt_count cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for SubscriptionEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("SubscriptionEvent: id is required".into());
        }
        if self.customer.is_empty() {
            return Err("SubscriptionEvent: customer is required".into());
        }
        if !["active", "past_due", "canceled", "incomplete", "incomplete_expired", "trialing", "unpaid", "paused"]
            .contains(&self.status.as_str())
        {
            return Err("SubscriptionEvent: invalid status".into());
        }
        if self.plan_amount < 0 {
            return Err("SubscriptionEvent: plan_amount cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for RefundEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("RefundEvent: id is required".into());
        }
        if self.amount <= 0 {
            return Err("RefundEvent: amount must be positive".into());
        }
        if self.charge.is_empty() {
            return Err("RefundEvent: charge is required".into());
        }
        if self.currency.len() != 3 {
            return Err("RefundEvent: currency must be 3 characters".into());
        }
        Ok(())
    }
}

impl Validatable for DisputeEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("DisputeEvent: id is required".into());
        }
        if self.amount <= 0 {
            return Err("DisputeEvent: amount must be positive".into());
        }
        if self.charge.is_empty() {
            return Err("DisputeEvent: charge is required".into());
        }
        if self.reason.is_empty() {
            return Err("DisputeEvent: reason is required".into());
        }
        Ok(())
    }
}

impl Validatable for ChargeEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("ChargeEvent: id is required".into());
        }
        if self.amount < 0 {
            return Err("ChargeEvent: amount cannot be negative".into());
        }
        if self.amount_captured > self.amount {
            return Err("ChargeEvent: amount_captured exceeds amount".into());
        }
        if self.amount_refunded > self.amount {
            return Err("ChargeEvent: amount_refunded exceeds amount".into());
        }
        if self.currency.len() != 3 {
            return Err("ChargeEvent: currency must be 3 characters".into());
        }
        Ok(())
    }
}

// --- Custom event types ---

impl Validatable for CustomJsonEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("CustomJsonEvent: id is required".into());
        }
        if self.source.is_empty() {
            return Err("CustomJsonEvent: source is required".into());
        }
        if self.event_type.is_empty() {
            return Err("CustomJsonEvent: event_type is required".into());
        }
        if self.schema_version.is_empty() {
            return Err("CustomJsonEvent: schema_version is required".into());
        }
        if self.max_retries < 0 {
            return Err("CustomJsonEvent: max_retries cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for AlertEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("AlertEvent: id is required".into());
        }
        if self.alert_name.is_empty() {
            return Err("AlertEvent: alert_name is required".into());
        }
        if !["critical", "high", "medium", "low", "info"].contains(&self.severity.as_str()) {
            return Err("AlertEvent: invalid severity level".into());
        }
        if self.fingerprint.is_empty() {
            return Err("AlertEvent: fingerprint is required".into());
        }
        if self.escalation_level < 0 {
            return Err("AlertEvent: escalation_level cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for MetricEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("MetricEvent: id is required".into());
        }
        if self.metric_name.is_empty() {
            return Err("MetricEvent: metric_name is required".into());
        }
        if self.namespace.is_empty() {
            return Err("MetricEvent: namespace is required".into());
        }
        if self.period_seconds <= 0 {
            return Err("MetricEvent: period_seconds must be positive".into());
        }
        if self.sample_rate <= 0.0 || self.sample_rate > 1.0 {
            return Err("MetricEvent: sample_rate must be between 0 and 1".into());
        }
        Ok(())
    }
}

impl Validatable for AuditEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("AuditEvent: id is required".into());
        }
        if self.actor_id.is_empty() {
            return Err("AuditEvent: actor_id is required".into());
        }
        if self.action.is_empty() {
            return Err("AuditEvent: action is required".into());
        }
        if self.resource_type.is_empty() {
            return Err("AuditEvent: resource_type is required".into());
        }
        if self.resource_id.is_empty() {
            return Err("AuditEvent: resource_id is required".into());
        }
        if !["success", "failure", "denied", "error"].contains(&self.result.as_str()) {
            return Err("AuditEvent: invalid result value".into());
        }
        Ok(())
    }
}

// --- CSV event types ---

impl Validatable for CsvTransactionRow {
    fn validate(&self) -> Result<(), String> {
        if self.transaction_id.is_empty() {
            return Err("CsvTransactionRow: transaction_id is required".into());
        }
        if self.account_id.is_empty() {
            return Err("CsvTransactionRow: account_id is required".into());
        }
        if self.currency.len() != 3 {
            return Err("CsvTransactionRow: currency must be 3 characters".into());
        }
        if self.category.is_empty() {
            return Err("CsvTransactionRow: category is required".into());
        }
        Ok(())
    }
}

impl Validatable for CsvInventoryRow {
    fn validate(&self) -> Result<(), String> {
        if self.sku.is_empty() {
            return Err("CsvInventoryRow: sku is required".into());
        }
        if self.product_name.is_empty() {
            return Err("CsvInventoryRow: product_name is required".into());
        }
        if self.quantity_on_hand < 0 {
            return Err("CsvInventoryRow: quantity_on_hand cannot be negative".into());
        }
        if self.unit_cost < 0.0 || self.unit_price < 0.0 {
            return Err("CsvInventoryRow: costs/prices cannot be negative".into());
        }
        if self.reorder_point < 0 {
            return Err("CsvInventoryRow: reorder_point cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for CsvUserActivityRow {
    fn validate(&self) -> Result<(), String> {
        if self.event_id.is_empty() {
            return Err("CsvUserActivityRow: event_id is required".into());
        }
        if self.user_id.is_empty() {
            return Err("CsvUserActivityRow: user_id is required".into());
        }
        if self.session_id.is_empty() {
            return Err("CsvUserActivityRow: session_id is required".into());
        }
        if self.event_type.is_empty() {
            return Err("CsvUserActivityRow: event_type is required".into());
        }
        if let Some(d) = self.duration_ms {
            if d < 0 {
                return Err("CsvUserActivityRow: duration_ms cannot be negative".into());
            }
        }
        Ok(())
    }
}
