use crate::sources::github::*;
use crate::sources::stripe::*;
use crate::sources::custom::*;
use crate::sources::csv::*;
use crate::sources::cloud::*;
use crate::sources::observability::*;
use crate::sources::commerce::*;

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

// --- Cloud event types ---

impl Validatable for Ec2InstanceEvent {
    fn validate(&self) -> Result<(), String> {
        if self.instance_id.is_empty() {
            return Err("Ec2InstanceEvent: instance_id is required".into());
        }
        if self.region.is_empty() {
            return Err("Ec2InstanceEvent: region is required".into());
        }
        if self.instance_type.is_empty() {
            return Err("Ec2InstanceEvent: instance_type is required".into());
        }
        if self.vpc_id.is_empty() {
            return Err("Ec2InstanceEvent: vpc_id is required".into());
        }
        if !["pending", "running", "shutting-down", "terminated", "stopping", "stopped"]
            .contains(&self.state.as_str())
        {
            return Err("Ec2InstanceEvent: invalid state".into());
        }
        Ok(())
    }
}

impl Validatable for S3BucketEvent {
    fn validate(&self) -> Result<(), String> {
        if self.bucket_name.is_empty() {
            return Err("S3BucketEvent: bucket_name is required".into());
        }
        if self.object_key.is_empty() {
            return Err("S3BucketEvent: object_key is required".into());
        }
        if self.object_size < 0 {
            return Err("S3BucketEvent: object_size cannot be negative".into());
        }
        if self.etag.is_empty() {
            return Err("S3BucketEvent: etag is required".into());
        }
        if self.region.is_empty() {
            return Err("S3BucketEvent: region is required".into());
        }
        Ok(())
    }
}

impl Validatable for LambdaInvocationEvent {
    fn validate(&self) -> Result<(), String> {
        if self.function_name.is_empty() {
            return Err("LambdaInvocationEvent: function_name is required".into());
        }
        if self.request_id.is_empty() {
            return Err("LambdaInvocationEvent: request_id is required".into());
        }
        if self.memory_size_mb <= 0 {
            return Err("LambdaInvocationEvent: memory_size_mb must be positive".into());
        }
        if self.timeout_seconds <= 0 {
            return Err("LambdaInvocationEvent: timeout_seconds must be positive".into());
        }
        if self.duration_ms < 0.0 {
            return Err("LambdaInvocationEvent: duration_ms cannot be negative".into());
        }
        if self.max_memory_used_mb < 0 {
            return Err("LambdaInvocationEvent: max_memory_used_mb cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for CloudWatchAlarmEvent {
    fn validate(&self) -> Result<(), String> {
        if self.alarm_name.is_empty() {
            return Err("CloudWatchAlarmEvent: alarm_name is required".into());
        }
        if self.alarm_arn.is_empty() {
            return Err("CloudWatchAlarmEvent: alarm_arn is required".into());
        }
        if !["OK", "ALARM", "INSUFFICIENT_DATA"].contains(&self.state_value.as_str()) {
            return Err("CloudWatchAlarmEvent: invalid state_value".into());
        }
        if self.metric_name.is_empty() {
            return Err("CloudWatchAlarmEvent: metric_name is required".into());
        }
        if self.period <= 0 {
            return Err("CloudWatchAlarmEvent: period must be positive".into());
        }
        if self.evaluation_periods <= 0 {
            return Err("CloudWatchAlarmEvent: evaluation_periods must be positive".into());
        }
        Ok(())
    }
}

impl Validatable for RdsEvent {
    fn validate(&self) -> Result<(), String> {
        if self.db_instance_id.is_empty() {
            return Err("RdsEvent: db_instance_id is required".into());
        }
        if self.engine.is_empty() {
            return Err("RdsEvent: engine is required".into());
        }
        if self.db_instance_class.is_empty() {
            return Err("RdsEvent: db_instance_class is required".into());
        }
        if self.allocated_storage_gb <= 0 {
            return Err("RdsEvent: allocated_storage_gb must be positive".into());
        }
        if self.region.is_empty() {
            return Err("RdsEvent: region is required".into());
        }
        if self.backup_retention_period < 0 {
            return Err("RdsEvent: backup_retention_period cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for EcsTaskEvent {
    fn validate(&self) -> Result<(), String> {
        if self.task_arn.is_empty() {
            return Err("EcsTaskEvent: task_arn is required".into());
        }
        if self.cluster_arn.is_empty() {
            return Err("EcsTaskEvent: cluster_arn is required".into());
        }
        if self.task_definition_arn.is_empty() {
            return Err("EcsTaskEvent: task_definition_arn is required".into());
        }
        if !["FARGATE", "EC2", "EXTERNAL"].contains(&self.launch_type.as_str()) {
            return Err("EcsTaskEvent: invalid launch_type".into());
        }
        if self.region.is_empty() {
            return Err("EcsTaskEvent: region is required".into());
        }
        Ok(())
    }
}

impl Validatable for SqsMessageEvent {
    fn validate(&self) -> Result<(), String> {
        if self.message_id.is_empty() {
            return Err("SqsMessageEvent: message_id is required".into());
        }
        if self.queue_url.is_empty() {
            return Err("SqsMessageEvent: queue_url is required".into());
        }
        if self.queue_name.is_empty() {
            return Err("SqsMessageEvent: queue_name is required".into());
        }
        if self.body.is_empty() {
            return Err("SqsMessageEvent: body is required".into());
        }
        if self.approximate_receive_count < 0 {
            return Err("SqsMessageEvent: approximate_receive_count cannot be negative".into());
        }
        if self.delay_seconds < 0 {
            return Err("SqsMessageEvent: delay_seconds cannot be negative".into());
        }
        if self.visibility_timeout_seconds < 0 {
            return Err("SqsMessageEvent: visibility_timeout_seconds cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for SnsNotificationEvent {
    fn validate(&self) -> Result<(), String> {
        if self.notification_id.is_empty() {
            return Err("SnsNotificationEvent: notification_id is required".into());
        }
        if self.topic_arn.is_empty() {
            return Err("SnsNotificationEvent: topic_arn is required".into());
        }
        if self.message.is_empty() {
            return Err("SnsNotificationEvent: message is required".into());
        }
        if self.message_id.is_empty() {
            return Err("SnsNotificationEvent: message_id is required".into());
        }
        if self.notification_type.is_empty() {
            return Err("SnsNotificationEvent: notification_type is required".into());
        }
        if self.signature_version.is_empty() {
            return Err("SnsNotificationEvent: signature_version is required".into());
        }
        Ok(())
    }
}

// --- Observability event types ---

impl Validatable for LogEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("LogEvent: id is required".into());
        }
        if self.message.is_empty() {
            return Err("LogEvent: message is required".into());
        }
        if !["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL"]
            .contains(&self.level.as_str())
        {
            return Err("LogEvent: invalid level".into());
        }
        if self.service_name.is_empty() {
            return Err("LogEvent: service_name is required".into());
        }
        if self.hostname.is_empty() {
            return Err("LogEvent: hostname is required".into());
        }
        if self.process_id <= 0 {
            return Err("LogEvent: process_id must be positive".into());
        }
        Ok(())
    }
}

impl Validatable for TraceSpanEvent {
    fn validate(&self) -> Result<(), String> {
        if self.trace_id.is_empty() {
            return Err("TraceSpanEvent: trace_id is required".into());
        }
        if self.span_id.is_empty() {
            return Err("TraceSpanEvent: span_id is required".into());
        }
        if self.operation_name.is_empty() {
            return Err("TraceSpanEvent: operation_name is required".into());
        }
        if self.service_name.is_empty() {
            return Err("TraceSpanEvent: service_name is required".into());
        }
        if !["CLIENT", "SERVER", "PRODUCER", "CONSUMER", "INTERNAL"]
            .contains(&self.span_kind.as_str())
        {
            return Err("TraceSpanEvent: invalid span_kind".into());
        }
        if self.duration_ns < 0 {
            return Err("TraceSpanEvent: duration_ns cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for MetricDatapointEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("MetricDatapointEvent: id is required".into());
        }
        if self.metric_name.is_empty() {
            return Err("MetricDatapointEvent: metric_name is required".into());
        }
        if !["gauge", "counter", "histogram", "summary", "exponential_histogram"]
            .contains(&self.metric_type.as_str())
        {
            return Err("MetricDatapointEvent: invalid metric_type".into());
        }
        if self.namespace.is_empty() {
            return Err("MetricDatapointEvent: namespace is required".into());
        }
        if self.count < 0 {
            return Err("MetricDatapointEvent: count cannot be negative".into());
        }
        if self.service_name.is_empty() {
            return Err("MetricDatapointEvent: service_name is required".into());
        }
        Ok(())
    }
}

impl Validatable for IncidentEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("IncidentEvent: id is required".into());
        }
        if self.title.is_empty() {
            return Err("IncidentEvent: title is required".into());
        }
        if self.incident_number <= 0 {
            return Err("IncidentEvent: incident_number must be positive".into());
        }
        if !["triggered", "acknowledged", "resolved", "postmortem"]
            .contains(&self.status.as_str())
        {
            return Err("IncidentEvent: invalid status".into());
        }
        if !["critical", "high", "medium", "low"].contains(&self.severity.as_str()) {
            return Err("IncidentEvent: invalid severity".into());
        }
        if self.impact_level.is_empty() {
            return Err("IncidentEvent: impact_level is required".into());
        }
        Ok(())
    }
}

impl Validatable for PagerDutyAlertEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("PagerDutyAlertEvent: id is required".into());
        }
        if self.incident_key.is_empty() {
            return Err("PagerDutyAlertEvent: incident_key is required".into());
        }
        if self.service_id.is_empty() {
            return Err("PagerDutyAlertEvent: service_id is required".into());
        }
        if self.title.is_empty() {
            return Err("PagerDutyAlertEvent: title is required".into());
        }
        if !["triggered", "acknowledged", "resolved"].contains(&self.status.as_str()) {
            return Err("PagerDutyAlertEvent: invalid status".into());
        }
        if !["high", "low"].contains(&self.urgency.as_str()) {
            return Err("PagerDutyAlertEvent: invalid urgency".into());
        }
        if self.alert_count < 0 {
            return Err("PagerDutyAlertEvent: alert_count cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for GrafanaAlertEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("GrafanaAlertEvent: id is required".into());
        }
        if self.rule_name.is_empty() {
            return Err("GrafanaAlertEvent: rule_name is required".into());
        }
        if self.title.is_empty() {
            return Err("GrafanaAlertEvent: title is required".into());
        }
        if !["alerting", "ok", "no_data", "paused", "pending"]
            .contains(&self.state.as_str())
        {
            return Err("GrafanaAlertEvent: invalid state".into());
        }
        if self.dashboard_id <= 0 {
            return Err("GrafanaAlertEvent: dashboard_id must be positive".into());
        }
        if self.frequency_seconds <= 0 {
            return Err("GrafanaAlertEvent: frequency_seconds must be positive".into());
        }
        Ok(())
    }
}

impl Validatable for DatadogEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("DatadogEvent: id is required".into());
        }
        if self.title.is_empty() {
            return Err("DatadogEvent: title is required".into());
        }
        if self.host.is_empty() {
            return Err("DatadogEvent: host is required".into());
        }
        if !["normal", "low"].contains(&self.priority.as_str()) {
            return Err("DatadogEvent: invalid priority".into());
        }
        if !["error", "warning", "info", "success", "user_update", "recommendation", "snapshot"]
            .contains(&self.alert_type.as_str())
        {
            return Err("DatadogEvent: invalid alert_type".into());
        }
        if self.source_type_name.is_empty() {
            return Err("DatadogEvent: source_type_name is required".into());
        }
        Ok(())
    }
}

impl Validatable for SentryErrorEvent {
    fn validate(&self) -> Result<(), String> {
        if self.event_id.is_empty() {
            return Err("SentryErrorEvent: event_id is required".into());
        }
        if self.project_id <= 0 {
            return Err("SentryErrorEvent: project_id must be positive".into());
        }
        if self.project_slug.is_empty() {
            return Err("SentryErrorEvent: project_slug is required".into());
        }
        if self.platform.is_empty() {
            return Err("SentryErrorEvent: platform is required".into());
        }
        if !["fatal", "error", "warning", "info", "debug"].contains(&self.level.as_str()) {
            return Err("SentryErrorEvent: invalid level".into());
        }
        if self.title.is_empty() {
            return Err("SentryErrorEvent: title is required".into());
        }
        if self.environment.is_empty() {
            return Err("SentryErrorEvent: environment is required".into());
        }
        Ok(())
    }
}

// --- Commerce event types ---

impl Validatable for OrderEvent {
    fn validate(&self) -> Result<(), String> {
        if self.order_id.is_empty() {
            return Err("OrderEvent: order_id is required".into());
        }
        if self.customer_id.is_empty() {
            return Err("OrderEvent: customer_id is required".into());
        }
        if self.total_amount < 0.0 {
            return Err("OrderEvent: total_amount cannot be negative".into());
        }
        if self.currency.len() != 3 {
            return Err("OrderEvent: currency must be 3 characters".into());
        }
        if !["pending", "confirmed", "processing", "shipped", "delivered", "cancelled", "refunded"]
            .contains(&self.status.as_str())
        {
            return Err("OrderEvent: invalid status".into());
        }
        if self.payment_method.is_empty() {
            return Err("OrderEvent: payment_method is required".into());
        }
        Ok(())
    }
}

impl Validatable for ShipmentEvent {
    fn validate(&self) -> Result<(), String> {
        if self.shipment_id.is_empty() {
            return Err("ShipmentEvent: shipment_id is required".into());
        }
        if self.order_id.is_empty() {
            return Err("ShipmentEvent: order_id is required".into());
        }
        if self.carrier.is_empty() {
            return Err("ShipmentEvent: carrier is required".into());
        }
        if self.tracking_number.is_empty() {
            return Err("ShipmentEvent: tracking_number is required".into());
        }
        if self.weight_kg < 0.0 {
            return Err("ShipmentEvent: weight_kg cannot be negative".into());
        }
        if self.shipping_cost < 0.0 {
            return Err("ShipmentEvent: shipping_cost cannot be negative".into());
        }
        if self.package_count <= 0 {
            return Err("ShipmentEvent: package_count must be positive".into());
        }
        Ok(())
    }
}

impl Validatable for InventoryChangeEvent {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("InventoryChangeEvent: id is required".into());
        }
        if self.sku.is_empty() {
            return Err("InventoryChangeEvent: sku is required".into());
        }
        if self.product_id.is_empty() {
            return Err("InventoryChangeEvent: product_id is required".into());
        }
        if self.warehouse_id.is_empty() {
            return Err("InventoryChangeEvent: warehouse_id is required".into());
        }
        if !["receipt", "shipment", "adjustment", "transfer", "return", "damage", "count"]
            .contains(&self.change_type.as_str())
        {
            return Err("InventoryChangeEvent: invalid change_type".into());
        }
        if self.unit_cost < 0.0 {
            return Err("InventoryChangeEvent: unit_cost cannot be negative".into());
        }
        if self.performed_by.is_empty() {
            return Err("InventoryChangeEvent: performed_by is required".into());
        }
        Ok(())
    }
}

impl Validatable for ReturnEvent {
    fn validate(&self) -> Result<(), String> {
        if self.return_id.is_empty() {
            return Err("ReturnEvent: return_id is required".into());
        }
        if self.order_id.is_empty() {
            return Err("ReturnEvent: order_id is required".into());
        }
        if self.customer_id.is_empty() {
            return Err("ReturnEvent: customer_id is required".into());
        }
        if self.refund_amount < 0.0 {
            return Err("ReturnEvent: refund_amount cannot be negative".into());
        }
        if self.currency.len() != 3 {
            return Err("ReturnEvent: currency must be 3 characters".into());
        }
        if !["requested", "approved", "received", "inspected", "refunded", "rejected"]
            .contains(&self.status.as_str())
        {
            return Err("ReturnEvent: invalid status".into());
        }
        if self.reason_code.is_empty() {
            return Err("ReturnEvent: reason_code is required".into());
        }
        Ok(())
    }
}

impl Validatable for ReviewEvent {
    fn validate(&self) -> Result<(), String> {
        if self.review_id.is_empty() {
            return Err("ReviewEvent: review_id is required".into());
        }
        if self.product_id.is_empty() {
            return Err("ReviewEvent: product_id is required".into());
        }
        if self.customer_id.is_empty() {
            return Err("ReviewEvent: customer_id is required".into());
        }
        if self.rating < 0.0 || self.rating > 5.0 {
            return Err("ReviewEvent: rating must be between 0 and 5".into());
        }
        if !["pending", "approved", "rejected", "flagged"]
            .contains(&self.status.as_str())
        {
            return Err("ReviewEvent: invalid status".into());
        }
        if self.helpful_votes < 0 || self.not_helpful_votes < 0 {
            return Err("ReviewEvent: vote counts cannot be negative".into());
        }
        Ok(())
    }
}

impl Validatable for CouponEvent {
    fn validate(&self) -> Result<(), String> {
        if self.coupon_id.is_empty() {
            return Err("CouponEvent: coupon_id is required".into());
        }
        if self.code.is_empty() {
            return Err("CouponEvent: code is required".into());
        }
        if self.discount_value < 0.0 {
            return Err("CouponEvent: discount_value cannot be negative".into());
        }
        if !["percentage", "fixed_amount", "free_shipping", "buy_x_get_y"]
            .contains(&self.discount_type.as_str())
        {
            return Err("CouponEvent: invalid discount_type".into());
        }
        if self.usage_count < 0 {
            return Err("CouponEvent: usage_count cannot be negative".into());
        }
        if self.created_by.is_empty() {
            return Err("CouponEvent: created_by is required".into());
        }
        Ok(())
    }
}
