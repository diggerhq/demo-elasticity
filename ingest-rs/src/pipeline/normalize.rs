use chrono::Utc;

use crate::unified::UnifiedEvent;
use crate::sources::github::*;
use crate::sources::stripe::*;
use crate::sources::custom::*;
use crate::sources::csv::*;
use crate::sources::cloud::*;
use crate::sources::observability::*;
use crate::sources::commerce::*;

/// Trait for normalizing a source-specific event into the unified format.
pub trait Normalizable {
    fn normalize(self) -> Result<UnifiedEvent, String>;
}

// --- GitHub event types ---

impl Normalizable for PushEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "github".into(),
            event_type: "push".into(),
            timestamp: self.timestamp,
            actor: self.pusher.clone(),
            action: if self.created { "branch_created" } else if self.deleted { "branch_deleted" } else { "push" }.into(),
            resource_type: "repository".into(),
            resource_id: self.repository.clone(),
            metadata: serde_json::json!({
                "ref": self.ref_name,
                "before": self.before,
                "after": self.after,
                "forced": self.forced,
                "commit_count": self.commits.len(),
            }),
            tags: vec!["github".into(), "push".into(), "vcs".into()],
            severity: "info".into(),
            correlation_id: format!("gh-push-{}", self.after),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for PullRequestEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: format!("gh-pr-{}", self.id),
            source: "github".into(),
            event_type: "pull_request".into(),
            timestamp: self.created_at,
            actor: self.user.clone(),
            action: self.action.clone(),
            resource_type: "pull_request".into(),
            resource_id: format!("{}#{}", self.repository, self.number),
            metadata: serde_json::json!({
                "title": self.title,
                "state": self.state,
                "merged": self.merged,
                "draft": self.draft,
                "head_ref": self.head_ref,
                "base_ref": self.base_ref,
                "additions": self.additions,
                "deletions": self.deletions,
                "changed_files": self.changed_files,
                "labels": self.labels,
                "reviewers": self.reviewers,
            }),
            tags: vec!["github".into(), "pull_request".into(), "code_review".into()],
            severity: "info".into(),
            correlation_id: format!("gh-pr-{}-{}", self.repository, self.number),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for IssueEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: format!("gh-issue-{}", self.id),
            source: "github".into(),
            event_type: "issue".into(),
            timestamp: self.created_at,
            actor: self.user.clone(),
            action: self.action.clone(),
            resource_type: "issue".into(),
            resource_id: format!("{}#{}", self.repository, self.number),
            metadata: serde_json::json!({
                "title": self.title,
                "state": self.state,
                "labels": self.labels,
                "assignees": self.assignees,
                "comments_count": self.comments_count,
                "is_pull_request": self.is_pull_request,
                "locked": self.locked,
            }),
            tags: vec!["github".into(), "issue".into(), "tracking".into()],
            severity: "info".into(),
            correlation_id: format!("gh-issue-{}-{}", self.repository, self.number),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for ReleaseEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: format!("gh-release-{}", self.id),
            source: "github".into(),
            event_type: "release".into(),
            timestamp: self.created_at,
            actor: self.author.clone(),
            action: self.action.clone(),
            resource_type: "release".into(),
            resource_id: format!("{}@{}", self.repository, self.tag_name),
            metadata: serde_json::json!({
                "tag_name": self.tag_name,
                "name": self.name,
                "draft": self.draft,
                "prerelease": self.prerelease,
                "target_commitish": self.target_commitish,
                "asset_count": self.assets.len(),
            }),
            tags: vec!["github".into(), "release".into(), "deployment".into()],
            severity: if self.prerelease { "info" } else { "low" }.into(),
            correlation_id: format!("gh-release-{}-{}", self.repository, self.tag_name),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for DeploymentEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: format!("gh-deploy-{}", self.id),
            source: "github".into(),
            event_type: "deployment".into(),
            timestamp: self.created_at,
            actor: self.creator.clone(),
            action: self.action.clone(),
            resource_type: "deployment".into(),
            resource_id: format!("{}:{}", self.repository, self.environment),
            metadata: serde_json::json!({
                "environment": self.environment,
                "ref": self.ref_name,
                "sha": self.sha,
                "task": self.task,
                "production": self.production_environment,
                "transient": self.transient_environment,
            }),
            tags: vec!["github".into(), "deployment".into(), self.environment.clone()],
            severity: if self.production_environment { "high" } else { "info" }.into(),
            correlation_id: format!("gh-deploy-{}-{}", self.repository, self.sha),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for CheckRunEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.conclusion.as_deref() {
            Some("failure") | Some("timed_out") => "high",
            Some("cancelled") | Some("action_required") => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: format!("gh-check-{}", self.id),
            source: "github".into(),
            event_type: "check_run".into(),
            timestamp: self.started_at,
            actor: self.app_name.clone(),
            action: self.action.clone(),
            resource_type: "check_run".into(),
            resource_id: format!("check-{}", self.id),
            metadata: serde_json::json!({
                "name": self.name,
                "status": self.status,
                "conclusion": self.conclusion,
                "head_sha": self.head_sha,
                "check_suite_id": self.check_suite_id,
                "annotations_count": self.output_annotations_count,
            }),
            tags: vec!["github".into(), "ci".into(), "check_run".into()],
            severity: severity.into(),
            correlation_id: format!("gh-check-suite-{}", self.check_suite_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for WorkflowRunEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.conclusion.as_deref() {
            Some("failure") | Some("timed_out") => "high",
            Some("cancelled") => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: format!("gh-wf-{}", self.id),
            source: "github".into(),
            event_type: "workflow_run".into(),
            timestamp: self.run_started_at,
            actor: self.triggering_actor.clone(),
            action: self.action.clone(),
            resource_type: "workflow_run".into(),
            resource_id: format!("wf-{}-run-{}", self.workflow_id, self.run_number),
            metadata: serde_json::json!({
                "name": self.name,
                "status": self.status,
                "conclusion": self.conclusion,
                "head_branch": self.head_branch,
                "head_sha": self.head_sha,
                "event": self.event,
                "run_number": self.run_number,
                "run_attempt": self.run_attempt,
                "path": self.path,
            }),
            tags: vec!["github".into(), "ci".into(), "workflow".into()],
            severity: severity.into(),
            correlation_id: format!("gh-wf-{}-{}", self.workflow_id, self.run_number),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

// --- Stripe event types ---

impl Normalizable for PaymentEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "stripe".into(),
            event_type: "payment".into(),
            timestamp: self.created,
            actor: self.customer.clone().unwrap_or_else(|| "anonymous".into()),
            action: format!("payment_{}", self.status),
            resource_type: "payment_intent".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "amount": self.amount,
                "amount_received": self.amount_received,
                "currency": self.currency,
                "payment_method": self.payment_method,
                "capture_method": self.capture_method,
                "payment_method_types": self.payment_method_types,
                "livemode": self.livemode,
            }),
            tags: vec!["stripe".into(), "payment".into(), self.currency.clone()],
            severity: if self.status == "requires_action" { "medium" } else { "info" }.into(),
            correlation_id: format!("stripe-pay-{}", self.id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for InvoiceEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "stripe".into(),
            event_type: "invoice".into(),
            timestamp: self.created,
            actor: self.customer.clone(),
            action: format!("invoice_{}", self.status),
            resource_type: "invoice".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "amount_due": self.amount_due,
                "amount_paid": self.amount_paid,
                "amount_remaining": self.amount_remaining,
                "currency": self.currency,
                "paid": self.paid,
                "attempted": self.attempted,
                "attempt_count": self.attempt_count,
                "billing_reason": self.billing_reason,
                "collection_method": self.collection_method,
                "subtotal": self.subtotal,
                "total": self.total,
            }),
            tags: vec!["stripe".into(), "invoice".into(), "billing".into()],
            severity: if !self.paid && self.attempt_count > 1 { "high" } else { "info" }.into(),
            correlation_id: format!("stripe-inv-{}", self.id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for SubscriptionEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.status.as_str() {
            "past_due" | "unpaid" => "high",
            "canceled" | "incomplete_expired" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "stripe".into(),
            event_type: "subscription".into(),
            timestamp: self.created,
            actor: self.customer.clone(),
            action: format!("subscription_{}", self.status),
            resource_type: "subscription".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "plan_id": self.plan_id,
                "plan_amount": self.plan_amount,
                "plan_currency": self.plan_currency,
                "plan_interval": self.plan_interval,
                "cancel_at_period_end": self.cancel_at_period_end,
                "quantity": self.quantity,
                "collection_method": self.collection_method,
            }),
            tags: vec!["stripe".into(), "subscription".into(), self.plan_interval.clone()],
            severity: severity.into(),
            correlation_id: format!("stripe-sub-{}", self.id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for RefundEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "stripe".into(),
            event_type: "refund".into(),
            timestamp: self.created,
            actor: "system".into(),
            action: format!("refund_{}", self.status),
            resource_type: "refund".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "amount": self.amount,
                "currency": self.currency,
                "charge": self.charge,
                "reason": self.reason,
                "payment_intent": self.payment_intent,
            }),
            tags: vec!["stripe".into(), "refund".into(), self.currency.clone()],
            severity: "medium".into(),
            correlation_id: format!("stripe-refund-{}", self.charge),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for DisputeEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "stripe".into(),
            event_type: "dispute".into(),
            timestamp: self.created,
            actor: "customer".into(),
            action: format!("dispute_{}", self.status),
            resource_type: "dispute".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "amount": self.amount,
                "currency": self.currency,
                "charge": self.charge,
                "reason": self.reason,
                "is_charge_refundable": self.is_charge_refundable,
                "network_reason_code": self.network_reason_code,
            }),
            tags: vec!["stripe".into(), "dispute".into(), "risk".into()],
            severity: "high".into(),
            correlation_id: format!("stripe-dispute-{}", self.charge),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for ChargeEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = if self.disputed {
            "high"
        } else if self.refunded {
            "medium"
        } else {
            "info"
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "stripe".into(),
            event_type: "charge".into(),
            timestamp: self.created,
            actor: self.customer.clone().unwrap_or_else(|| "anonymous".into()),
            action: if self.captured { "charge_captured" } else { "charge_pending" }.into(),
            resource_type: "charge".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "amount": self.amount,
                "amount_captured": self.amount_captured,
                "amount_refunded": self.amount_refunded,
                "currency": self.currency,
                "paid": self.paid,
                "captured": self.captured,
                "refunded": self.refunded,
                "disputed": self.disputed,
                "payment_intent": self.payment_intent,
                "payment_method": self.payment_method,
            }),
            tags: vec!["stripe".into(), "charge".into(), self.currency.clone()],
            severity: severity.into(),
            correlation_id: format!("stripe-charge-{}", self.id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

// --- Custom event types ---

impl Normalizable for CustomJsonEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: self.source.clone(),
            event_type: self.event_type.clone(),
            timestamp: self.timestamp,
            actor: self.metadata.get("actor")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            action: self.event_type.clone(),
            resource_type: "custom".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "schema_version": self.schema_version,
                "priority": self.priority,
                "ttl_seconds": self.ttl_seconds,
                "retry_count": self.retry_count,
                "max_retries": self.max_retries,
                "idempotency_key": self.idempotency_key,
                "payload": self.payload,
            }),
            tags: self.tags.clone(),
            severity: match self.priority {
                p if p >= 8 => "critical".into(),
                p if p >= 5 => "high".into(),
                p if p >= 3 => "medium".into(),
                _ => "low".into(),
            },
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            correlation_id: self.correlation_id.unwrap_or_else(|| format!("custom-{}", self.id)),
            processed_at: Utc::now(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for AlertEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: self.source.clone(),
            event_type: "alert".into(),
            timestamp: self.triggered_at,
            actor: self.acknowledged_by.clone().unwrap_or_else(|| "system".into()),
            action: format!("alert_{}", self.status),
            resource_type: "alert".into(),
            resource_id: self.fingerprint.clone(),
            metadata: serde_json::json!({
                "alert_name": self.alert_name,
                "message": self.message,
                "status": self.status,
                "escalation_level": self.escalation_level,
                "threshold_value": self.threshold_value,
                "current_value": self.current_value,
                "labels": self.labels,
                "notification_channels": self.notification_channels,
            }),
            tags: {
                let mut tags = vec!["alert".into(), self.severity.clone()];
                tags.extend(self.notification_channels.iter().cloned());
                tags
            },
            severity: self.severity.clone(),
            correlation_id: format!("alert-{}", self.fingerprint),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for MetricEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: format!("{}:{}", self.source_host, self.namespace),
            event_type: "metric".into(),
            timestamp: self.timestamp,
            actor: self.source_host.clone(),
            action: "metric_reported".into(),
            resource_type: "metric".into(),
            resource_id: format!("{}/{}", self.namespace, self.metric_name),
            metadata: serde_json::json!({
                "metric_name": self.metric_name,
                "value": self.value,
                "unit": self.unit,
                "aggregation_type": self.aggregation_type,
                "period_seconds": self.period_seconds,
                "count": self.count,
                "min": self.min_value,
                "max": self.max_value,
                "sum": self.sum_value,
                "dimensions": self.dimensions,
                "is_monotonic": self.is_monotonic,
            }),
            tags: {
                let mut tags = self.tags.clone();
                tags.push(self.source_region.clone());
                tags.push(self.source_environment.clone());
                tags
            },
            severity: "info".into(),
            correlation_id: format!("metric-{}-{}", self.namespace, self.metric_name),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for AuditEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.result.as_str() {
            "denied" => "high",
            "failure" | "error" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: format!("audit:{}", self.organization_id),
            event_type: "audit".into(),
            timestamp: self.timestamp,
            actor: self.actor_email.clone(),
            action: self.action.clone(),
            resource_type: self.resource_type.clone(),
            resource_id: self.resource_id.clone(),
            metadata: serde_json::json!({
                "actor_id": self.actor_id,
                "actor_ip": self.actor_ip,
                "result": self.result,
                "reason": self.reason,
                "organization_id": self.organization_id,
                "project_id": self.project_id,
                "session_id": self.session_id,
                "request_id": self.request_id,
                "mfa_used": self.mfa_used,
                "risk_score": self.risk_score,
                "changes": self.changes,
            }),
            tags: {
                let mut tags = vec!["audit".into(), self.result.clone()];
                tags.extend(self.compliance_tags.iter().cloned());
                tags
            },
            severity: severity.into(),
            correlation_id: format!("audit-session-{}", self.session_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

// --- CSV event types ---

impl Normalizable for CsvTransactionRow {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.transaction_id.clone(),
            source: "csv:transactions".into(),
            event_type: "transaction".into(),
            timestamp: self.timestamp,
            actor: self.account_id.clone(),
            action: self.transaction_type.clone(),
            resource_type: "transaction".into(),
            resource_id: self.transaction_id.clone(),
            metadata: serde_json::json!({
                "amount": self.amount,
                "currency": self.currency,
                "status": self.status,
                "category": self.category,
                "subcategory": self.subcategory,
                "merchant_name": self.merchant_name,
                "merchant_category_code": self.merchant_category_code,
                "balance_after": self.balance_after,
                "fee_amount": self.fee_amount,
                "is_recurring": self.is_recurring,
                "risk_score": self.risk_score,
            }),
            tags: {
                let mut tags = vec!["csv".into(), "transaction".into(), self.category.clone()];
                tags.extend(self.tags.iter().cloned());
                tags
            },
            severity: if self.risk_score.unwrap_or(0.0) > 0.8 { "high" } else { "info" }.into(),
            correlation_id: format!("csv-txn-{}", self.account_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for CsvInventoryRow {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = if self.quantity_available <= 0 {
            "high"
        } else if self.quantity_available <= self.reorder_point {
            "medium"
        } else {
            "info"
        };
        Ok(UnifiedEvent {
            id: format!("inv-{}-{}", self.warehouse_id, self.sku),
            source: "csv:inventory".into(),
            event_type: "inventory_snapshot".into(),
            timestamp: self.last_counted_at.unwrap_or_else(Utc::now),
            actor: self.supplier_name.clone(),
            action: "inventory_update".into(),
            resource_type: "inventory_item".into(),
            resource_id: self.sku.clone(),
            metadata: serde_json::json!({
                "product_name": self.product_name,
                "category": self.category,
                "quantity_on_hand": self.quantity_on_hand,
                "quantity_reserved": self.quantity_reserved,
                "quantity_available": self.quantity_available,
                "reorder_point": self.reorder_point,
                "unit_cost": self.unit_cost,
                "unit_price": self.unit_price,
                "warehouse_id": self.warehouse_id,
                "supplier_id": self.supplier_id,
                "is_active": self.is_active,
                "is_hazardous": self.is_hazardous,
            }),
            tags: {
                let mut tags = vec!["csv".into(), "inventory".into(), self.category.clone()];
                tags.extend(self.tags.iter().cloned());
                tags
            },
            severity: severity.into(),
            correlation_id: format!("csv-inv-{}", self.warehouse_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for CsvUserActivityRow {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.event_id.clone(),
            source: "csv:user_activity".into(),
            event_type: "user_activity".into(),
            timestamp: self.timestamp,
            actor: self.user_id.clone(),
            action: self.event_type.clone(),
            resource_type: "page".into(),
            resource_id: self.page_url.clone().unwrap_or_else(|| "unknown".into()),
            metadata: serde_json::json!({
                "session_id": self.session_id,
                "duration_ms": self.duration_ms,
                "device_type": self.device_type,
                "os_name": self.os_name,
                "browser_name": self.browser_name,
                "country_code": self.country_code,
                "region": self.region,
                "city": self.city,
                "utm_source": self.utm_source,
                "utm_medium": self.utm_medium,
                "utm_campaign": self.utm_campaign,
                "is_authenticated": self.is_authenticated,
                "ab_test_variants": self.ab_test_variants,
            }),
            tags: {
                let mut tags = vec!["csv".into(), "user_activity".into(), self.device_type.clone()];
                tags.extend(self.feature_flags.iter().cloned());
                tags
            },
            severity: "info".into(),
            correlation_id: format!("csv-session-{}", self.session_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

// --- Cloud event types ---

impl Normalizable for Ec2InstanceEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.state.as_str() {
            "terminated" | "shutting-down" => "medium",
            "stopping" | "stopped" => "low",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: format!("ec2-{}", self.instance_id),
            source: "aws:ec2".into(),
            event_type: "ec2_instance".into(),
            timestamp: self.launch_time,
            actor: self.iam_role.clone().unwrap_or_else(|| "unknown".into()),
            action: format!("instance_{}", self.state),
            resource_type: "ec2_instance".into(),
            resource_id: self.instance_id.clone(),
            metadata: serde_json::json!({
                "instance_type": self.instance_type,
                "region": self.region,
                "availability_zone": self.availability_zone,
                "state": self.state,
                "vpc_id": self.vpc_id,
                "subnet_id": self.subnet_id,
                "image_id": self.image_id,
                "private_ip": self.private_ip,
                "public_ip": self.public_ip,
                "cpu_utilization": self.cpu_utilization,
                "memory_utilization": self.memory_utilization,
                "network_in_bytes": self.network_in_bytes,
                "network_out_bytes": self.network_out_bytes,
                "tenancy": self.tenancy,
                "monitoring_state": self.monitoring_state,
                "auto_scaling_group": self.auto_scaling_group,
            }),
            tags: {
                let mut tags = vec!["aws".into(), "ec2".into(), self.region.clone(), self.instance_type.clone()];
                for sg in &self.security_groups {
                    tags.push(sg.clone());
                }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("ec2-{}-{}", self.vpc_id, self.instance_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for S3BucketEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: format!("s3-{}", self.request_id),
            source: "aws:s3".into(),
            event_type: "s3_object".into(),
            timestamp: self.timestamp,
            actor: self.requester_id.clone(),
            action: self.event_type.clone(),
            resource_type: "s3_object".into(),
            resource_id: format!("{}/{}", self.bucket_name, self.object_key),
            metadata: serde_json::json!({
                "bucket_name": self.bucket_name,
                "object_key": self.object_key,
                "object_size": self.object_size,
                "etag": self.etag,
                "storage_class": self.storage_class,
                "content_type": self.content_type,
                "version_id": self.version_id,
                "is_delete_marker": self.is_delete_marker,
                "encryption_type": self.encryption_type,
                "replication_status": self.replication_status,
                "source_ip": self.source_ip,
                "transfer_acceleration": self.transfer_acceleration,
            }),
            tags: vec!["aws".into(), "s3".into(), self.region.clone(), self.storage_class.clone()],
            severity: if self.is_delete_marker { "medium" } else { "info" }.into(),
            correlation_id: format!("s3-{}-{}", self.bucket_name, self.request_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for LambdaInvocationEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = if self.error_type.is_some() {
            "high"
        } else if self.cold_start {
            "low"
        } else {
            "info"
        };
        Ok(UnifiedEvent {
            id: format!("lambda-{}", self.request_id),
            source: "aws:lambda".into(),
            event_type: "lambda_invocation".into(),
            timestamp: self.timestamp,
            actor: self.event_source_arn.clone().unwrap_or_else(|| "direct".into()),
            action: format!("invoke_{}", self.invocation_type),
            resource_type: "lambda_function".into(),
            resource_id: format!("{}:{}", self.function_name, self.function_version),
            metadata: serde_json::json!({
                "function_name": self.function_name,
                "function_version": self.function_version,
                "runtime": self.runtime,
                "handler": self.handler,
                "memory_size_mb": self.memory_size_mb,
                "max_memory_used_mb": self.max_memory_used_mb,
                "duration_ms": self.duration_ms,
                "billed_duration_ms": self.billed_duration_ms,
                "init_duration_ms": self.init_duration_ms,
                "status_code": self.status_code,
                "cold_start": self.cold_start,
                "error_type": self.error_type,
                "error_message": self.error_message,
                "log_group": self.log_group,
                "layers": self.layers,
            }),
            tags: {
                let mut tags = vec!["aws".into(), "lambda".into(), self.region.clone(), self.runtime.clone()];
                if self.cold_start { tags.push("cold_start".into()); }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("lambda-{}-{}", self.function_name, self.request_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for CloudWatchAlarmEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.state_value.as_str() {
            "ALARM" => "high",
            "INSUFFICIENT_DATA" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: format!("cw-alarm-{}-{}", self.alarm_name, self.state_updated_timestamp.timestamp()),
            source: "aws:cloudwatch".into(),
            event_type: "cloudwatch_alarm".into(),
            timestamp: self.state_updated_timestamp,
            actor: format!("account:{}", self.account_id),
            action: format!("alarm_state_{}", self.state_value.to_lowercase()),
            resource_type: "cloudwatch_alarm".into(),
            resource_id: self.alarm_arn.clone(),
            metadata: serde_json::json!({
                "alarm_name": self.alarm_name,
                "state_value": self.state_value,
                "previous_state_value": self.previous_state_value,
                "state_reason": self.state_reason,
                "metric_name": self.metric_name,
                "namespace": self.namespace,
                "statistic": self.statistic,
                "threshold": self.threshold,
                "comparison_operator": self.comparison_operator,
                "period": self.period,
                "evaluation_periods": self.evaluation_periods,
                "treat_missing_data": self.treat_missing_data,
                "actions_enabled": self.actions_enabled,
                "dimensions": self.dimensions,
            }),
            tags: vec!["aws".into(), "cloudwatch".into(), "alarm".into(), self.region.clone(), self.namespace.clone()],
            severity: severity.into(),
            correlation_id: format!("cw-alarm-{}", self.alarm_arn),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for RdsEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.status.as_str() {
            "failed" | "incompatible-restore" | "storage-full" => "critical",
            "stopping" | "stopped" | "deleting" => "high",
            "maintenance" | "modifying" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: format!("rds-{}-{}", self.db_instance_id, self.timestamp.timestamp()),
            source: "aws:rds".into(),
            event_type: "rds_instance".into(),
            timestamp: self.timestamp,
            actor: self.master_username.clone(),
            action: format!("rds_{}", self.event_type),
            resource_type: "rds_instance".into(),
            resource_id: self.db_instance_id.clone(),
            metadata: serde_json::json!({
                "engine": self.engine,
                "engine_version": self.engine_version,
                "db_instance_class": self.db_instance_class,
                "allocated_storage_gb": self.allocated_storage_gb,
                "multi_az": self.multi_az,
                "status": self.status,
                "storage_type": self.storage_type,
                "storage_encrypted": self.storage_encrypted,
                "cpu_utilization": self.cpu_utilization,
                "free_storage_bytes": self.free_storage_bytes,
                "connections_count": self.connections_count,
                "read_iops": self.read_iops,
                "write_iops": self.write_iops,
                "replica_lag_seconds": self.replica_lag_seconds,
                "performance_insights_enabled": self.performance_insights_enabled,
                "deletion_protection": self.deletion_protection,
            }),
            tags: vec!["aws".into(), "rds".into(), self.region.clone(), self.engine.clone()],
            severity: severity.into(),
            correlation_id: format!("rds-{}", self.db_instance_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for EcsTaskEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.last_status.as_str() {
            "STOPPED" if self.stopping_reason.is_some() => "high",
            "STOPPED" => "medium",
            "DEACTIVATING" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: format!("ecs-{}", self.task_arn.rsplit('/').next().unwrap_or(&self.task_arn)),
            source: "aws:ecs".into(),
            event_type: "ecs_task".into(),
            timestamp: self.timestamp,
            actor: self.started_by.clone().unwrap_or_else(|| "ecs-service".into()),
            action: format!("task_{}", self.last_status.to_lowercase()),
            resource_type: "ecs_task".into(),
            resource_id: self.task_arn.clone(),
            metadata: serde_json::json!({
                "cluster_arn": self.cluster_arn,
                "task_definition_arn": self.task_definition_arn,
                "launch_type": self.launch_type,
                "desired_status": self.desired_status,
                "last_status": self.last_status,
                "health_status": self.health_status,
                "cpu": self.cpu,
                "memory": self.memory,
                "platform_version": self.platform_version,
                "connectivity": self.connectivity,
                "stopping_reason": self.stopping_reason,
                "group": self.group,
                "enable_execute_command": self.enable_execute_command,
                "container_count": self.containers.len(),
            }),
            tags: vec!["aws".into(), "ecs".into(), self.region.clone(), self.launch_type.clone()],
            severity: severity.into(),
            correlation_id: format!("ecs-cluster-{}", self.cluster_arn),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for SqsMessageEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: format!("sqs-{}", self.message_id),
            source: "aws:sqs".into(),
            event_type: "sqs_message".into(),
            timestamp: self.sent_timestamp,
            actor: self.sender_id.clone(),
            action: "message_sent".into(),
            resource_type: "sqs_queue".into(),
            resource_id: self.queue_url.clone(),
            metadata: serde_json::json!({
                "queue_name": self.queue_name,
                "body_md5": self.body_md5,
                "approximate_receive_count": self.approximate_receive_count,
                "message_group_id": self.message_group_id,
                "message_deduplication_id": self.message_deduplication_id,
                "delay_seconds": self.delay_seconds,
                "visibility_timeout_seconds": self.visibility_timeout_seconds,
                "is_fifo": self.is_fifo,
                "content_based_deduplication": self.content_based_deduplication,
                "dead_letter_queue_arn": self.dead_letter_queue_arn,
                "redrive_count": self.redrive_count,
                "encryption_type": self.encryption_type,
            }),
            tags: {
                let mut tags = vec!["aws".into(), "sqs".into(), self.region.clone()];
                if self.is_fifo { tags.push("fifo".into()); }
                tags
            },
            severity: if self.approximate_receive_count > 3 { "medium" } else { "info" }.into(),
            correlation_id: format!("sqs-{}-{}", self.queue_name, self.message_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for SnsNotificationEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: format!("sns-{}", self.notification_id),
            source: "aws:sns".into(),
            event_type: "sns_notification".into(),
            timestamp: self.timestamp,
            actor: self.topic_name.clone(),
            action: format!("notification_{}", self.notification_type.to_lowercase()),
            resource_type: "sns_topic".into(),
            resource_id: self.topic_arn.clone(),
            metadata: serde_json::json!({
                "topic_name": self.topic_name,
                "message_id": self.message_id,
                "subject": self.subject,
                "notification_type": self.notification_type,
                "subscription_arn": self.subscription_arn,
                "is_fifo": self.is_fifo,
                "message_group_id": self.message_group_id,
                "protocol": self.protocol,
                "endpoint": self.endpoint,
                "signature_version": self.signature_version,
                "content_based_deduplication": self.content_based_deduplication,
            }),
            tags: {
                let mut tags = vec!["aws".into(), "sns".into(), self.region.clone(), self.protocol.clone()];
                if self.is_fifo { tags.push("fifo".into()); }
                tags
            },
            severity: "info".into(),
            correlation_id: format!("sns-{}-{}", self.topic_name, self.message_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

// --- Observability event types ---

impl Normalizable for LogEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.level.as_str() {
            "FATAL" => "critical",
            "ERROR" => "high",
            "WARN" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: format!("log:{}", self.service_name),
            event_type: "log".into(),
            timestamp: self.timestamp,
            actor: self.hostname.clone(),
            action: format!("log_{}", self.level.to_lowercase()),
            resource_type: "service".into(),
            resource_id: self.service_name.clone(),
            metadata: serde_json::json!({
                "level": self.level,
                "message": self.message,
                "logger_name": self.logger_name,
                "thread_name": self.thread_name,
                "process_id": self.process_id,
                "service_version": self.service_version,
                "environment": self.environment,
                "trace_id": self.trace_id,
                "span_id": self.span_id,
                "file_name": self.file_name,
                "line_number": self.line_number,
                "function_name": self.function_name,
                "exception_type": self.exception_type,
                "exception_message": self.exception_message,
                "severity_number": self.severity_number,
                "body_bytes": self.body_bytes,
            }),
            tags: {
                let mut tags = vec!["log".into(), self.level.clone().to_lowercase(), self.service_name.clone(), self.environment.clone()];
                for (k, v) in &self.tags {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: severity.into(),
            correlation_id: self.trace_id.clone().unwrap_or_else(|| format!("log-{}", self.id)),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for TraceSpanEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.status_code.as_str() {
            "ERROR" => "high",
            "UNSET" => "low",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: format!("span-{}", self.span_id),
            source: format!("trace:{}", self.service_name),
            event_type: "trace_span".into(),
            timestamp: self.start_time,
            actor: self.service_name.clone(),
            action: self.operation_name.clone(),
            resource_type: "span".into(),
            resource_id: format!("{}:{}", self.trace_id, self.span_id),
            metadata: serde_json::json!({
                "trace_id": self.trace_id,
                "parent_span_id": self.parent_span_id,
                "span_kind": self.span_kind,
                "duration_ns": self.duration_ns,
                "status_code": self.status_code,
                "status_message": self.status_message,
                "instrumentation_library": self.instrumentation_library,
                "http_method": self.http_method,
                "http_url": self.http_url,
                "http_status_code": self.http_status_code,
                "db_system": self.db_system,
                "rpc_system": self.rpc_system,
                "rpc_service": self.rpc_service,
                "net_peer_name": self.net_peer_name,
                "net_peer_port": self.net_peer_port,
                "error": self.error,
                "event_count": self.events.len(),
                "link_count": self.links.len(),
            }),
            tags: {
                let mut tags = vec!["trace".into(), self.span_kind.clone().to_lowercase(), self.service_name.clone(), self.environment.clone()];
                if self.error { tags.push("error".into()); }
                tags
            },
            severity: severity.into(),
            correlation_id: self.trace_id.clone(),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for MetricDatapointEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: format!("metric:{}", self.service_name),
            event_type: "metric_datapoint".into(),
            timestamp: self.timestamp,
            actor: self.hostname.clone(),
            action: "metric_reported".into(),
            resource_type: "metric".into(),
            resource_id: format!("{}/{}", self.namespace, self.metric_name),
            metadata: serde_json::json!({
                "metric_name": self.metric_name,
                "metric_type": self.metric_type,
                "namespace": self.namespace,
                "value": self.value,
                "count": self.count,
                "sum": self.sum,
                "min": self.min,
                "max": self.max,
                "unit": self.unit,
                "is_monotonic": self.is_monotonic,
                "aggregation_temporality": self.aggregation_temporality,
                "environment": self.environment,
                "region": self.region,
                "dimensions": self.dimensions,
                "flags": self.flags,
                "histogram_buckets": self.histogram_bucket_counts.len(),
            }),
            tags: {
                let mut tags = vec!["metric".into(), self.metric_type.clone(), self.service_name.clone(), self.environment.clone(), self.region.clone()];
                for (k, v) in &self.tags {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: "info".into(),
            correlation_id: format!("metric-{}-{}", self.namespace, self.metric_name),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for IncidentEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.severity.as_str() {
            "critical" => "critical",
            "high" => "high",
            "medium" => "medium",
            _ => "low",
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "incident_management".into(),
            event_type: "incident".into(),
            timestamp: self.created_at,
            actor: self.commander.clone().unwrap_or_else(|| "unassigned".into()),
            action: format!("incident_{}", self.status),
            resource_type: "incident".into(),
            resource_id: format!("INC-{}", self.incident_number),
            metadata: serde_json::json!({
                "title": self.title,
                "description": self.description,
                "status": self.status,
                "severity": self.severity,
                "priority": self.priority,
                "impact_level": self.impact_level,
                "customer_impact": self.customer_impact,
                "customer_impact_duration_minutes": self.customer_impact_duration_minutes,
                "affected_services": self.affected_services,
                "affected_environments": self.affected_environments,
                "responders": self.responders,
                "duration_seconds": self.duration_seconds,
                "time_to_detect_seconds": self.time_to_detect_seconds,
                "time_to_resolve_seconds": self.time_to_resolve_seconds,
                "postmortem_url": self.postmortem_url,
                "source_alerts": self.source_alerts,
            }),
            tags: {
                let mut tags = vec!["incident".into(), self.severity.clone(), self.status.clone()];
                tags.extend(self.labels.iter().cloned());
                tags.extend(self.affected_services.iter().cloned());
                tags
            },
            severity: severity.into(),
            correlation_id: format!("incident-{}", self.id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for PagerDutyAlertEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.urgency.as_str() {
            "high" => "high",
            _ => "medium",
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "pagerduty".into(),
            event_type: "pagerduty_alert".into(),
            timestamp: self.created_at,
            actor: self.last_status_change_by.clone().unwrap_or_else(|| "system".into()),
            action: format!("alert_{}", self.status),
            resource_type: "pagerduty_incident".into(),
            resource_id: self.incident_key.clone(),
            metadata: serde_json::json!({
                "title": self.title,
                "description": self.description,
                "service_id": self.service_id,
                "service_name": self.service_name,
                "escalation_policy_id": self.escalation_policy_id,
                "escalation_policy_name": self.escalation_policy_name,
                "urgency": self.urgency,
                "status": self.status,
                "trigger_type": self.trigger_type,
                "trigger_summary": self.trigger_summary,
                "alert_count": self.alert_count,
                "incident_number": self.incident_number,
                "assigned_to": self.assigned_to,
                "acknowledged_by": self.acknowledged_by,
                "resolved_by": self.resolved_by,
                "integration_name": self.integration_name,
                "html_url": self.html_url,
            }),
            tags: {
                let mut tags = vec!["pagerduty".into(), self.urgency.clone(), self.status.clone(), self.service_name.clone()];
                for (k, v) in &self.tags {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("pd-{}", self.incident_key),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for GrafanaAlertEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.state.as_str() {
            "alerting" => match self.severity.as_str() {
                "critical" => "critical",
                "high" => "high",
                _ => "medium",
            },
            "no_data" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "grafana".into(),
            event_type: "grafana_alert".into(),
            timestamp: self.new_state_date,
            actor: format!("dashboard:{}", self.dashboard_uid),
            action: format!("alert_{}", self.state),
            resource_type: "grafana_alert_rule".into(),
            resource_id: format!("alert-{}", self.alert_id),
            metadata: serde_json::json!({
                "rule_name": self.rule_name,
                "title": self.title,
                "state": self.state,
                "previous_state": self.previous_state,
                "severity": self.severity,
                "message": self.message,
                "dashboard_id": self.dashboard_id,
                "dashboard_uid": self.dashboard_uid,
                "panel_id": self.panel_id,
                "org_id": self.org_id,
                "frequency_seconds": self.frequency_seconds,
                "silenced": self.silenced,
                "no_data_state": self.no_data_state,
                "values": self.values,
                "conditions": self.conditions,
                "folder_title": self.folder_title,
            }),
            tags: {
                let mut tags = vec!["grafana".into(), "alert".into(), self.state.clone(), self.severity.clone()];
                tags.extend(self.notification_channels.iter().cloned());
                for (k, v) in &self.labels {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("grafana-alert-{}", self.alert_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for DatadogEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.alert_type.as_str() {
            "error" => "high",
            "warning" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: "datadog".into(),
            event_type: "datadog_event".into(),
            timestamp: self.date_happened,
            actor: self.host.clone(),
            action: self.alert_type.clone(),
            resource_type: "datadog_event".into(),
            resource_id: self.id.clone(),
            metadata: serde_json::json!({
                "title": self.title,
                "text": self.text,
                "priority": self.priority,
                "host": self.host,
                "device_name": self.device_name,
                "alert_type": self.alert_type,
                "source_type_name": self.source_type_name,
                "aggregation_key": self.aggregation_key,
                "monitor_id": self.monitor_id,
                "monitor_groups": self.monitor_groups,
                "transition": self.transition,
                "org_id": self.org_id,
                "org_name": self.org_name,
                "is_aggregate": self.is_aggregate,
                "multi": self.multi,
                "resource": self.resource,
            }),
            tags: {
                let mut tags = self.tags.clone();
                tags.push("datadog".into());
                tags.push(self.source_type_name.clone());
                tags
            },
            severity: severity.into(),
            correlation_id: self.aggregation_key.clone().unwrap_or_else(|| format!("dd-{}", self.id)),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for SentryErrorEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.level.as_str() {
            "fatal" => "critical",
            "error" => "high",
            "warning" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.event_id.clone(),
            source: format!("sentry:{}:{}", self.organization_slug, self.project_slug),
            event_type: "sentry_error".into(),
            timestamp: self.timestamp,
            actor: self.user_id.clone().unwrap_or_else(|| "anonymous".into()),
            action: format!("error_{}", self.level),
            resource_type: "sentry_issue".into(),
            resource_id: self.group_id.map(|g| format!("group-{}", g)).unwrap_or_else(|| self.event_id.clone()),
            metadata: serde_json::json!({
                "title": self.title,
                "message": self.message,
                "platform": self.platform,
                "level": self.level,
                "logger": self.logger,
                "transaction": self.transaction,
                "release": self.release,
                "environment": self.environment,
                "exception_type": self.exception_type,
                "exception_value": self.exception_value,
                "culprit": self.culprit,
                "is_unhandled": self.is_unhandled,
                "sdk_name": self.sdk_name,
                "sdk_version": self.sdk_version,
                "request_url": self.request_url,
                "request_method": self.request_method,
                "user_email": self.user_email,
                "stacktrace_frame_count": self.stacktrace_frames.len(),
                "breadcrumb_count": self.breadcrumbs.len(),
            }),
            tags: {
                let mut tags = vec![
                    "sentry".into(),
                    self.level.clone(),
                    self.platform.clone(),
                    self.environment.clone(),
                ];
                if self.is_unhandled { tags.push("unhandled".into()); }
                for v in &self.fingerprint {
                    tags.push(v.clone());
                }
                tags
            },
            severity: severity.into(),
            correlation_id: self.group_id.map(|g| format!("sentry-group-{}", g)).unwrap_or_else(|| format!("sentry-{}", self.event_id)),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

// --- Commerce event types ---

impl Normalizable for OrderEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.status.as_str() {
            "cancelled" | "refunded" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.order_id.clone(),
            source: format!("commerce:{}", self.source_channel),
            event_type: "order".into(),
            timestamp: self.created_at,
            actor: self.customer_id.clone(),
            action: format!("order_{}", self.status),
            resource_type: "order".into(),
            resource_id: self.order_id.clone(),
            metadata: serde_json::json!({
                "customer_email": self.customer_email,
                "status": self.status,
                "currency": self.currency,
                "subtotal": self.subtotal,
                "tax_amount": self.tax_amount,
                "shipping_amount": self.shipping_amount,
                "discount_amount": self.discount_amount,
                "total_amount": self.total_amount,
                "payment_method": self.payment_method,
                "payment_status": self.payment_status,
                "shipping_method": self.shipping_method,
                "fulfillment_status": self.fulfillment_status,
                "source_channel": self.source_channel,
                "is_gift": self.is_gift,
                "coupon_codes": self.coupon_codes,
                "line_item_count": self.line_items.len(),
                "tracking_number": self.tracking_number,
            }),
            tags: {
                let mut tags = vec!["commerce".into(), "order".into(), self.status.clone(), self.source_channel.clone()];
                tags.extend(self.coupon_codes.iter().cloned());
                for (k, v) in &self.tags {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("order-{}", self.order_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for ShipmentEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.status.as_str() {
            "exception" | "failed" => "high",
            "returned" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.shipment_id.clone(),
            source: format!("commerce:shipping:{}", self.carrier),
            event_type: "shipment".into(),
            timestamp: self.created_at,
            actor: self.carrier.clone(),
            action: format!("shipment_{}", self.status),
            resource_type: "shipment".into(),
            resource_id: self.shipment_id.clone(),
            metadata: serde_json::json!({
                "order_id": self.order_id,
                "carrier": self.carrier,
                "carrier_service": self.carrier_service,
                "tracking_number": self.tracking_number,
                "tracking_url": self.tracking_url,
                "status": self.status,
                "weight_kg": self.weight_kg,
                "shipping_cost": self.shipping_cost,
                "insurance_cost": self.insurance_cost,
                "currency": self.currency,
                "package_count": self.package_count,
                "signature_required": self.signature_required,
                "is_return": self.is_return,
                "warehouse_id": self.warehouse_id,
                "estimated_delivery_date": self.estimated_delivery_date,
                "item_count": self.items.len(),
                "event_count": self.events.len(),
            }),
            tags: {
                let mut tags = vec!["commerce".into(), "shipment".into(), self.carrier.clone(), self.status.clone()];
                if self.is_return { tags.push("return".into()); }
                if self.signature_required { tags.push("signature_required".into()); }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("order-{}", self.order_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for InventoryChangeEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = if self.quantity_after <= 0 {
            "high"
        } else if self.change_type == "damage" {
            "medium"
        } else {
            "info"
        };
        Ok(UnifiedEvent {
            id: self.id.clone(),
            source: format!("commerce:inventory:{}", self.warehouse_id),
            event_type: "inventory_change".into(),
            timestamp: self.timestamp,
            actor: self.performed_by.clone(),
            action: format!("inventory_{}", self.change_type),
            resource_type: "inventory_item".into(),
            resource_id: format!("{}:{}", self.warehouse_id, self.sku),
            metadata: serde_json::json!({
                "sku": self.sku,
                "product_id": self.product_id,
                "product_name": self.product_name,
                "variant_id": self.variant_id,
                "change_type": self.change_type,
                "quantity_before": self.quantity_before,
                "quantity_after": self.quantity_after,
                "quantity_delta": self.quantity_delta,
                "reason": self.reason,
                "unit_cost": self.unit_cost,
                "total_value_change": self.total_value_change,
                "currency": self.currency,
                "lot_number": self.lot_number,
                "is_adjustment": self.is_adjustment,
                "approval_status": self.approval_status,
                "supplier_id": self.supplier_id,
                "purchase_order_id": self.purchase_order_id,
            }),
            tags: {
                let mut tags = vec!["commerce".into(), "inventory".into(), self.change_type.clone(), self.warehouse_id.clone()];
                for (k, v) in &self.tags {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("inv-{}-{}", self.warehouse_id, self.sku),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for ReturnEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = match self.status.as_str() {
            "rejected" => "high",
            "requested" | "approved" => "medium",
            _ => "info",
        };
        Ok(UnifiedEvent {
            id: self.return_id.clone(),
            source: "commerce:returns".into(),
            event_type: "return".into(),
            timestamp: self.created_at,
            actor: self.customer_id.clone(),
            action: format!("return_{}", self.status),
            resource_type: "return".into(),
            resource_id: self.return_id.clone(),
            metadata: serde_json::json!({
                "order_id": self.order_id,
                "customer_email": self.customer_email,
                "status": self.status,
                "return_type": self.return_type,
                "reason_code": self.reason_code,
                "reason_description": self.reason_description,
                "refund_amount": self.refund_amount,
                "restocking_fee": self.restocking_fee,
                "return_shipping_cost": self.return_shipping_cost,
                "currency": self.currency,
                "refund_method": self.refund_method,
                "refund_status": self.refund_status,
                "quality_inspection_status": self.quality_inspection_status,
                "is_exchange": self.is_exchange,
                "exchange_order_id": self.exchange_order_id,
                "condition": self.condition,
                "item_count": self.items.len(),
            }),
            tags: {
                let mut tags = vec!["commerce".into(), "return".into(), self.status.clone(), self.reason_code.clone()];
                if self.is_exchange { tags.push("exchange".into()); }
                for (k, v) in &self.tags {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: severity.into(),
            correlation_id: format!("order-{}", self.order_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for ReviewEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = if self.report_count > 3 {
            "medium"
        } else if self.rating <= 1.0 {
            "low"
        } else {
            "info"
        };
        Ok(UnifiedEvent {
            id: self.review_id.clone(),
            source: format!("commerce:reviews:{}", self.source_platform),
            event_type: "review".into(),
            timestamp: self.created_at,
            actor: self.customer_id.clone(),
            action: format!("review_{}", self.status),
            resource_type: "product_review".into(),
            resource_id: format!("review-{}", self.review_id),
            metadata: serde_json::json!({
                "product_id": self.product_id,
                "product_name": self.product_name,
                "variant_id": self.variant_id,
                "order_id": self.order_id,
                "rating": self.rating,
                "title": self.title,
                "status": self.status,
                "verified_purchase": self.verified_purchase,
                "helpful_votes": self.helpful_votes,
                "not_helpful_votes": self.not_helpful_votes,
                "report_count": self.report_count,
                "sentiment_score": self.sentiment_score,
                "language": self.language,
                "source_platform": self.source_platform,
                "has_response": self.response.is_some(),
                "media_count": self.media_urls.len(),
            }),
            tags: {
                let mut tags = vec!["commerce".into(), "review".into(), self.source_platform.clone(), self.status.clone()];
                if self.verified_purchase { tags.push("verified".into()); }
                tags.extend(self.pros.iter().map(|p| format!("pro:{}", p)));
                tags.extend(self.cons.iter().map(|c| format!("con:{}", c)));
                tags
            },
            severity: severity.into(),
            correlation_id: format!("product-{}", self.product_id),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}

impl Normalizable for CouponEvent {
    fn normalize(self) -> Result<UnifiedEvent, String> {
        let severity = if !self.is_active {
            "low"
        } else if self.usage_limit.map_or(false, |l| self.usage_count >= l) {
            "medium"
        } else {
            "info"
        };
        Ok(UnifiedEvent {
            id: self.coupon_id.clone(),
            source: "commerce:coupons".into(),
            event_type: "coupon".into(),
            timestamp: self.created_at,
            actor: self.created_by.clone(),
            action: if self.is_active { "coupon_active" } else { "coupon_inactive" }.into(),
            resource_type: "coupon".into(),
            resource_id: self.code.clone(),
            metadata: serde_json::json!({
                "code": self.code,
                "name": self.name,
                "description": self.description,
                "discount_type": self.discount_type,
                "discount_value": self.discount_value,
                "currency": self.currency,
                "minimum_order_amount": self.minimum_order_amount,
                "maximum_discount_amount": self.maximum_discount_amount,
                "usage_limit": self.usage_limit,
                "usage_count": self.usage_count,
                "is_active": self.is_active,
                "customer_eligibility": self.customer_eligibility,
                "first_order_only": self.first_order_only,
                "combinable": self.combinable,
                "auto_apply": self.auto_apply,
                "total_discount_given": self.total_discount_given,
                "conversion_rate": self.conversion_rate,
                "campaign_id": self.campaign_id,
            }),
            tags: {
                let mut tags = vec!["commerce".into(), "coupon".into(), self.discount_type.clone()];
                if self.first_order_only { tags.push("first_order".into()); }
                if self.auto_apply { tags.push("auto_apply".into()); }
                tags.extend(self.channel_restrictions.iter().cloned());
                for (k, v) in &self.tags {
                    tags.push(format!("{}:{}", k, v));
                }
                tags
            },
            severity: severity.into(),
            correlation_id: self.campaign_id.clone().unwrap_or_else(|| format!("coupon-{}", self.coupon_id)),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
            version: "1.0".into(),
        })
    }
}
