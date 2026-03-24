use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::pipeline::process_event;
use crate::sources::github::*;
use crate::sources::stripe::*;
use crate::sources::custom::*;
use crate::sources::csv::*;

/// Response for single-event endpoints.
#[derive(Debug, Serialize, Deserialize)]
pub struct SingleResponse {
    pub success: bool,
    pub data: Option<String>,
    pub error: Option<String>,
}

/// Response for batch endpoints.
/// NOTE: This is deliberately missing a `processed_at` field — that's the demo
/// issue the agent will fix.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchResponse {
    pub count: usize,
    pub results: Vec<String>,
}

/// Request wrapper for typed event ingestion.
#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// Request wrapper for batch ingestion.
#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub event_type: String,
    pub events: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// GitHub handlers
// ---------------------------------------------------------------------------

pub async fn ingest_github(
    Json(req): Json<IngestRequest>,
) -> Result<Json<SingleResponse>, (StatusCode, Json<SingleResponse>)> {
    let raw = serde_json::to_string(&req.payload).unwrap_or_default();

    let result = match req.event_type.as_str() {
        "push" => process_event::<PushEvent>(&raw),
        "pull_request" => process_event::<PullRequestEvent>(&raw),
        "issue" => process_event::<IssueEvent>(&raw),
        "release" => process_event::<ReleaseEvent>(&raw),
        "deployment" => process_event::<DeploymentEvent>(&raw),
        "check_run" => process_event::<CheckRunEvent>(&raw),
        "workflow_run" => process_event::<WorkflowRunEvent>(&raw),
        other => Err(format!("unknown github event type: {}", other)),
    };

    match result {
        Ok(data) => Ok(Json(SingleResponse {
            success: true,
            data: Some(data),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(SingleResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}

pub async fn ingest_github_batch(
    Json(req): Json<BatchRequest>,
) -> Result<Json<BatchResponse>, (StatusCode, Json<SingleResponse>)> {
    let mut results = Vec::new();

    for event_payload in &req.events {
        let raw = serde_json::to_string(event_payload).unwrap_or_default();
        let result = match req.event_type.as_str() {
            "push" => process_event::<PushEvent>(&raw),
            "pull_request" => process_event::<PullRequestEvent>(&raw),
            "issue" => process_event::<IssueEvent>(&raw),
            "release" => process_event::<ReleaseEvent>(&raw),
            "deployment" => process_event::<DeploymentEvent>(&raw),
            "check_run" => process_event::<CheckRunEvent>(&raw),
            "workflow_run" => process_event::<WorkflowRunEvent>(&raw),
            other => Err(format!("unknown github event type: {}", other)),
        };
        match result {
            Ok(data) => results.push(data),
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(SingleResponse {
                        success: false,
                        data: None,
                        error: Some(e),
                    }),
                ));
            }
        }
    }

    Ok(Json(BatchResponse {
        count: results.len(),
        results,
    }))
}

// ---------------------------------------------------------------------------
// Stripe handlers
// ---------------------------------------------------------------------------

pub async fn ingest_stripe(
    Json(req): Json<IngestRequest>,
) -> Result<Json<SingleResponse>, (StatusCode, Json<SingleResponse>)> {
    let raw = serde_json::to_string(&req.payload).unwrap_or_default();

    let result = match req.event_type.as_str() {
        "payment" => process_event::<PaymentEvent>(&raw),
        "invoice" => process_event::<InvoiceEvent>(&raw),
        "subscription" => process_event::<SubscriptionEvent>(&raw),
        "refund" => process_event::<RefundEvent>(&raw),
        "dispute" => process_event::<DisputeEvent>(&raw),
        "charge" => process_event::<ChargeEvent>(&raw),
        other => Err(format!("unknown stripe event type: {}", other)),
    };

    match result {
        Ok(data) => Ok(Json(SingleResponse {
            success: true,
            data: Some(data),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(SingleResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}

pub async fn ingest_stripe_batch(
    Json(req): Json<BatchRequest>,
) -> Result<Json<BatchResponse>, (StatusCode, Json<SingleResponse>)> {
    let mut results = Vec::new();

    for event_payload in &req.events {
        let raw = serde_json::to_string(event_payload).unwrap_or_default();
        let result = match req.event_type.as_str() {
            "payment" => process_event::<PaymentEvent>(&raw),
            "invoice" => process_event::<InvoiceEvent>(&raw),
            "subscription" => process_event::<SubscriptionEvent>(&raw),
            "refund" => process_event::<RefundEvent>(&raw),
            "dispute" => process_event::<DisputeEvent>(&raw),
            "charge" => process_event::<ChargeEvent>(&raw),
            other => Err(format!("unknown stripe event type: {}", other)),
        };
        match result {
            Ok(data) => results.push(data),
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(SingleResponse {
                        success: false,
                        data: None,
                        error: Some(e),
                    }),
                ));
            }
        }
    }

    Ok(Json(BatchResponse {
        count: results.len(),
        results,
    }))
}

// ---------------------------------------------------------------------------
// Custom handlers
// ---------------------------------------------------------------------------

pub async fn ingest_custom(
    Json(req): Json<IngestRequest>,
) -> Result<Json<SingleResponse>, (StatusCode, Json<SingleResponse>)> {
    let raw = serde_json::to_string(&req.payload).unwrap_or_default();

    let result = match req.event_type.as_str() {
        "custom_json" => process_event::<CustomJsonEvent>(&raw),
        "alert" => process_event::<AlertEvent>(&raw),
        "metric" => process_event::<MetricEvent>(&raw),
        "audit" => process_event::<AuditEvent>(&raw),
        other => Err(format!("unknown custom event type: {}", other)),
    };

    match result {
        Ok(data) => Ok(Json(SingleResponse {
            success: true,
            data: Some(data),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(SingleResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}

pub async fn ingest_custom_batch(
    Json(req): Json<BatchRequest>,
) -> Result<Json<BatchResponse>, (StatusCode, Json<SingleResponse>)> {
    let mut results = Vec::new();

    for event_payload in &req.events {
        let raw = serde_json::to_string(event_payload).unwrap_or_default();
        let result = match req.event_type.as_str() {
            "custom_json" => process_event::<CustomJsonEvent>(&raw),
            "alert" => process_event::<AlertEvent>(&raw),
            "metric" => process_event::<MetricEvent>(&raw),
            "audit" => process_event::<AuditEvent>(&raw),
            other => Err(format!("unknown custom event type: {}", other)),
        };
        match result {
            Ok(data) => results.push(data),
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(SingleResponse {
                        success: false,
                        data: None,
                        error: Some(e),
                    }),
                ));
            }
        }
    }

    Ok(Json(BatchResponse {
        count: results.len(),
        results,
    }))
}

// ---------------------------------------------------------------------------
// CSV handlers
// ---------------------------------------------------------------------------

pub async fn ingest_csv(
    Json(req): Json<IngestRequest>,
) -> Result<Json<SingleResponse>, (StatusCode, Json<SingleResponse>)> {
    let raw = serde_json::to_string(&req.payload).unwrap_or_default();

    let result = match req.event_type.as_str() {
        "transaction" => process_event::<CsvTransactionRow>(&raw),
        "inventory" => process_event::<CsvInventoryRow>(&raw),
        "user_activity" => process_event::<CsvUserActivityRow>(&raw),
        other => Err(format!("unknown csv event type: {}", other)),
    };

    match result {
        Ok(data) => Ok(Json(SingleResponse {
            success: true,
            data: Some(data),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(SingleResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}

pub async fn ingest_csv_batch(
    Json(req): Json<BatchRequest>,
) -> Result<Json<BatchResponse>, (StatusCode, Json<SingleResponse>)> {
    let mut results = Vec::new();

    for event_payload in &req.events {
        let raw = serde_json::to_string(event_payload).unwrap_or_default();
        let result = match req.event_type.as_str() {
            "transaction" => process_event::<CsvTransactionRow>(&raw),
            "inventory" => process_event::<CsvInventoryRow>(&raw),
            "user_activity" => process_event::<CsvUserActivityRow>(&raw),
            other => Err(format!("unknown csv event type: {}", other)),
        };
        match result {
            Ok(data) => results.push(data),
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(SingleResponse {
                        success: false,
                        data: None,
                        error: Some(e),
                    }),
                ));
            }
        }
    }

    Ok(Json(BatchResponse {
        count: results.len(),
        results,
    }))
}
