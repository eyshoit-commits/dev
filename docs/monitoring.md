# Monitoring and Telemetry

HelixDB emits metrics, logs, and structured telemetry events suitable for centralized
observability platforms.

## Metrics

Prometheus metrics are exposed via the `/api/braindb/metrics` endpoint.

| Metric | Type | Description |
|--------|------|-------------|
| `requests_total` | counter | Emitted by Tower HTTP tracing (status/route labels) |
| `helix_query_duration_ms` | histogram | Query execution latency (derive via tracing spans) |
| `telemetry_events_total` | counter | Number of events published (via log aggregation) |

Enable the Prometheus exporter by setting `telemetry.prometheus_endpoint` in the configuration
(default `0.0.0.0:9600`).

## Telemetry events

`TelemetryHub` publishes JSON events for:

- `query_executed`
- `plugin_registered`
- `document_inserted`
- `vector_search`

Ingest these events by forwarding structured logs to Loki, Elasticsearch, or Kafka.

## Logging

- Structured JSON logs available when `RUST_LOG=info`.
- Use `tracing-subscriber` filters (e.g. `RUST_LOG=helix_db=debug,hyper=info`).
- Enable TLS termination logs at the ingress to correlate client requests with HelixDB traces.

## Alerting recommendations

- Alert on sustained increases in `helix_query_duration_ms` or HTTP 5xx rate.
- Alert on missing `plugin.registered` events during deployments.
- Track LMDB disk utilization to avoid map-size exhaustion.
