# ingest-rs

Data ingestion service that normalizes events from multiple sources (GitHub webhooks, Stripe webhooks, custom HTTP payloads, CSV batch uploads) into a unified format via a generic transform pipeline.

The pipeline parses, validates, normalizes, enriches, and serializes events. Each stage is generic over the source event type, creating significant monomorphization pressure across ~20 distinct event structs.

## Build

```bash
cargo build
```

For single-threaded builds (predictable memory usage):

```bash
CARGO_BUILD_JOBS=1 cargo build
```

## Run

```bash
cargo run
```

Defaults to port 8080. Override with:

```bash
cargo run -- --port 3000
```

## Test

```bash
cargo test
```

## API

All endpoints accept `POST` with a JSON body containing `event_type` and `payload` fields.

| Endpoint | Sources |
|----------|---------|
| `POST /ingest/github` | push, pull_request, issue, release, deployment, check_run, workflow_run |
| `POST /ingest/stripe` | payment, invoice, subscription, refund, dispute, charge |
| `POST /ingest/custom` | custom_json, alert, metric, audit |
| `POST /ingest/csv` | transaction, inventory, user_activity |

Batch endpoints at `/ingest/{source}/batch` accept `event_type` and `events` (array of payloads).
