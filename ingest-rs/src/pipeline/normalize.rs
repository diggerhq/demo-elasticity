use chrono::Utc;

use crate::unified::UnifiedEvent;
use crate::sources::github::*;
use crate::sources::stripe::*;
use crate::sources::custom::*;
use crate::sources::csv::*;

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
            correlation_id: self.correlation_id.unwrap_or_else(|| format!("custom-{}", self.id)),
            processed_at: Utc::now(),
            raw_payload: serde_json::to_value(&self).unwrap_or_default(),
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
