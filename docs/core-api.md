# maid-core API Reference

Base URL: `http://<host>:43121/api/goose`

All endpoints require authentication via `Authorization: Bearer <JWT>` or `x-api-key` header issued by maid-apikeys. Scopes:

- `read:loadtests` – View status/history/metrics
- `write:loadtests` – Start or stop runs

## JSON Schema

`GET /api/goose/schema`

Returns the JSON Schema for Goose run configuration. Use this to power form generation inside the dashboard.

## Start a run

`POST /api/goose/run`

```json
{
  "config": {
    "target_base_url": "https://target.example",
    "users": 200,
    "hatch_rate": 50,
    "duration": { "seconds": 600 },
    "think_time_seconds": 3,
    "scenarios": [
      {
        "name": "checkout",
        "weight": 2,
        "transactions": [
          {
            "name": "get-root",
            "request": {
              "method": "GET",
              "path": "/"
            }
          }
        ]
      }
    ],
    "scheduler": "round-robin",
    "reports": { "formats": ["json", "csv", "html"] },
    "plugin_hints": { "tags": ["production", "canary"] }
  }
}
```

Response:

```json
{
  "run_id": "4f4199fc-3c20-4bb9-ae31-b6aa9d9a36a2",
  "status": {
    "active": true,
    "run_id": "4f4199fc-3c20-4bb9-ae31-b6aa9d9a36a2",
    "phase": "increase",
    "started_at": "2024-06-15T12:00:00Z"
  }
}
```

## Stop the active run

`POST /api/goose/stop`

Returns `202 Accepted` and triggers graceful shutdown.

## Engine status

`GET /api/goose/status`

```json
{
  "status": {
    "active": false,
    "run_id": "4f4199fc-3c20-4bb9-ae31-b6aa9d9a36a2",
    "phase": "completed",
    "started_at": "2024-06-15T12:00:00Z"
  }
}
```

## History

`GET /api/goose/history`

Returns a list of completed/past runs with timestamps and statuses. Use `run_id` to fetch reports from the filesystem (`reports/<run_id>/`).

## Metrics & logs streaming

`GET /api/goose/stream`

Upgrade to WebSocket. Each message is JSON encoded `StreamEnvelope`:

```json
{"Metrics":{"timestamp":"2024-06-15T12:01:00Z","cpu_usage":52.2,"ram_usage":43.1,"throughput_rps":410.5,"error_rate":0.5,"status_codes":{"200":950,"400":20,"500":5},"latency_p50_ms":70.4,"latency_p90_ms":180.2,"latency_p95_ms":210.5,"latency_p99_ms":340.4,"latency_p999_ms":489.0,"network_in_kbps":2300.0,"network_out_kbps":3150.0,"phase":"maintain"}}
{"Log":{"timestamp":"2024-06-15T12:01:05Z","level":"info","message":"run 4f4199fc... is healthy at 60 seconds"}}
{"Status":{"run_id":"4f4199fc-3c20-4bb9-ae31-b6aa9d9a36a2","phase":"maintain","active_users":200,"duration_seconds":60}}
```

## Reports

Upon completion the engine emits HTML (`report.html`), CSV (`report.csv`), and JSON (`report.json`) artifacts stored under `reports/<run_id>/` (path configurable via `Settings.persistence.report_dir`).

## Plugin integration

The core maintains a `PluginRegistry` for mistral.rs. Configure `plugin_bus.mistral_endpoint` and `plugin_bus.api_keys_endpoint` in `config.json` so the engine can:

- Forward recipe generation requests when operators use natural language scenario builders.
- Submit run summaries for AI-powered bottleneck analysis.
- Introspect credentials via maid-apikeys before executing privileged operations.

