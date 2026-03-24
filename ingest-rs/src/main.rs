mod handlers;
mod pipeline;
mod sources;
mod unified;
mod storage;

use axum::{routing::post, Router};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ingest-rs", about = "Data ingestion service with generic transform pipeline")]
struct Args {
    /// Port to bind the HTTP server to
    #[arg(short, long, default_value_t = 8080)]
    port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let app = Router::new()
        // Single-event endpoints
        .route("/ingest/github", post(handlers::ingest_github))
        .route("/ingest/stripe", post(handlers::ingest_stripe))
        .route("/ingest/custom", post(handlers::ingest_custom))
        .route("/ingest/csv", post(handlers::ingest_csv))
        .route("/ingest/cloud", post(handlers::ingest_cloud))
        .route("/ingest/observability", post(handlers::ingest_observability))
        .route("/ingest/commerce", post(handlers::ingest_commerce))
        // Batch endpoints
        .route("/ingest/github/batch", post(handlers::ingest_github_batch))
        .route("/ingest/stripe/batch", post(handlers::ingest_stripe_batch))
        .route("/ingest/custom/batch", post(handlers::ingest_custom_batch))
        .route("/ingest/csv/batch", post(handlers::ingest_csv_batch))
        .route("/ingest/cloud/batch", post(handlers::ingest_cloud_batch))
        .route("/ingest/observability/batch", post(handlers::ingest_observability_batch))
        .route("/ingest/commerce/batch", post(handlers::ingest_commerce_batch));

    let addr = format!("0.0.0.0:{}", args.port);
    tracing::info!("Starting ingest-rs on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
