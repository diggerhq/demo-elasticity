use ingest_rs::pipeline::process_event;
use ingest_rs::sources::github::PushEvent;
use ingest_rs::sources::stripe::PaymentEvent;
use ingest_rs::sources::custom::AlertEvent;
use ingest_rs::sources::csv::CsvTransactionRow;

#[test]
fn test_github_push_pipeline() {
    let raw = serde_json::json!({
        "id": "evt-push-001",
        "ref_name": "refs/heads/main",
        "before": "abc123def456abc123def456abc123def456abc1",
        "after": "def456abc123def456abc123def456abc123def4",
        "repository": "demo-org/ingest-rs",
        "pusher": "developer1",
        "created": false,
        "deleted": false,
        "forced": false,
        "commits": [{"id": "def456", "message": "fix: something"}],
        "head_commit": {"id": "def456", "message": "fix: something"},
        "timestamp": "2025-01-15T10:30:00Z"
    });

    let result = process_event::<PushEvent>(&serde_json::to_string(&raw).unwrap());
    assert!(result.is_ok(), "PushEvent pipeline failed: {:?}", result.err());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["source"], "github");
    assert_eq!(parsed["event_type"], "push");
    assert_eq!(parsed["actor"], "developer1");
    assert!(parsed["metadata"]["dedup_key"].is_string());
}

#[test]
fn test_stripe_payment_pipeline() {
    let raw = serde_json::json!({
        "id": "pi_test_123456",
        "object": "payment_intent",
        "amount": 2500,
        "amount_received": 2500,
        "currency": "usd",
        "customer": "cus_test_abc",
        "description": "Test payment",
        "status": "succeeded",
        "payment_method": "pm_card_visa",
        "payment_method_types": ["card"],
        "created": "2025-01-15T10:30:00Z",
        "livemode": false,
        "metadata": {"order_id": "ord-123"},
        "receipt_email": "test@example.com",
        "statement_descriptor": "TEST CHARGE",
        "capture_method": "automatic",
        "confirmation_method": "automatic",
        "client_secret": null,
        "last_payment_error": null,
        "next_action": null,
        "shipping": null,
        "charges": [],
        "application_fee_amount": null,
        "transfer_data": null,
        "on_behalf_of": null
    });

    let result = process_event::<PaymentEvent>(&serde_json::to_string(&raw).unwrap());
    assert!(result.is_ok(), "PaymentEvent pipeline failed: {:?}", result.err());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["source"], "stripe");
    assert_eq!(parsed["event_type"], "payment");
    assert_eq!(parsed["resource_id"], "pi_test_123456");
}

#[test]
fn test_custom_alert_pipeline() {
    let raw = serde_json::json!({
        "id": "alert-001",
        "alert_name": "HighCPUUsage",
        "severity": "critical",
        "source": "monitoring",
        "message": "CPU usage exceeded 95% for 5 minutes",
        "description": "Host web-01 CPU is at 97%",
        "triggered_at": "2025-01-15T10:30:00Z",
        "resolved_at": null,
        "acknowledged_at": null,
        "acknowledged_by": null,
        "status": "firing",
        "labels": {"host": "web-01", "env": "production"},
        "annotations": {"summary": "High CPU on web-01"},
        "fingerprint": "abc123fingerprint",
        "generator_url": "http://prometheus:9090/graph",
        "dashboard_url": "http://grafana/d/cpu",
        "runbook_url": "http://wiki/runbooks/high-cpu",
        "previous_severity": null,
        "escalation_level": 1,
        "notification_channels": ["slack", "pagerduty"],
        "related_alerts": [],
        "affected_resources": [{"type": "host", "id": "web-01"}],
        "threshold_value": 95.0,
        "current_value": 97.3,
        "evaluation_period_seconds": 300
    });

    let result = process_event::<AlertEvent>(&serde_json::to_string(&raw).unwrap());
    assert!(result.is_ok(), "AlertEvent pipeline failed: {:?}", result.err());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["source"], "monitoring");
    assert_eq!(parsed["event_type"], "alert");
    assert_eq!(parsed["severity"], "critical");
}

#[test]
fn test_csv_transaction_pipeline() {
    let raw = serde_json::json!({
        "transaction_id": "txn-001",
        "account_id": "acc-123",
        "counterparty_id": "acc-456",
        "amount": 150.75,
        "currency": "usd",
        "transaction_type": "debit",
        "status": "completed",
        "description": "Office supplies",
        "reference": "PO-2025-001",
        "category": "expenses",
        "subcategory": "office",
        "timestamp": "2025-01-15T10:30:00Z",
        "settled_at": "2025-01-15T12:00:00Z",
        "merchant_name": "Office Depot",
        "merchant_category_code": "5943",
        "balance_after": 8450.25,
        "fee_amount": 0.50,
        "exchange_rate": null,
        "original_amount": null,
        "original_currency": null,
        "metadata": {"department": "engineering"},
        "tags": ["office", "supplies"],
        "is_recurring": false,
        "risk_score": 0.1
    });

    let result = process_event::<CsvTransactionRow>(&serde_json::to_string(&raw).unwrap());
    assert!(result.is_ok(), "CsvTransactionRow pipeline failed: {:?}", result.err());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["source"], "csv:transactions");
    assert_eq!(parsed["event_type"], "transaction");
    assert_eq!(parsed["actor"], "acc-123");
}

#[test]
fn test_validation_failure() {
    // PushEvent with empty ref_name should fail validation
    let raw = serde_json::json!({
        "id": "evt-bad-001",
        "ref_name": "",
        "before": "abc123",
        "after": "def456",
        "repository": "demo-org/test",
        "pusher": "someone",
        "created": false,
        "deleted": false,
        "forced": false,
        "commits": [],
        "head_commit": null,
        "timestamp": "2025-01-15T10:30:00Z"
    });

    let result = process_event::<PushEvent>(&serde_json::to_string(&raw).unwrap());
    assert!(result.is_err(), "Expected validation error for empty ref_name");
    assert!(result.unwrap_err().contains("validation error"));
}

#[test]
fn test_output_is_valid_json() {
    let raw = serde_json::json!({
        "id": "pi_json_test",
        "object": "payment_intent",
        "amount": 1000,
        "amount_received": 1000,
        "currency": "eur",
        "customer": "cus_test",
        "description": null,
        "status": "succeeded",
        "payment_method": "pm_card_mastercard",
        "payment_method_types": ["card"],
        "created": "2025-01-15T10:30:00Z",
        "livemode": false,
        "metadata": {},
        "receipt_email": null,
        "statement_descriptor": null,
        "capture_method": "automatic",
        "confirmation_method": "automatic",
        "client_secret": null,
        "last_payment_error": null,
        "next_action": null,
        "shipping": null,
        "charges": [],
        "application_fee_amount": null,
        "transfer_data": null,
        "on_behalf_of": null
    });

    let result = process_event::<PaymentEvent>(&serde_json::to_string(&raw).unwrap()).unwrap();

    // Output should be valid JSON (single-line JSON Lines)
    for line in result.lines() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        assert!(parsed.is_ok(), "Output line is not valid JSON: {}", line);
    }
}
