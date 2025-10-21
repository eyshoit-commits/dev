# MAID Architecture Overview

The MAID platform orchestrates three cooperating services that expose secure APIs and a plugin bus for orchestrating load testing, analytics, and administration.

```
+----------------+      +-------------------+      +-------------------+
| maid-apikeys   |<---->|  maid-core        |<---->| maid-mistral      |
| Auth & RBAC    |      | Goose load engine |      | AI recipes + LLM  |
+----------------+      +-------------------+      +-------------------+
       ^                         ^                           ^
       |                         |                           |
  Dashboard UI           Plugin bus events           AI analysis hooks
```

## Core responsibilities

- **maid-core**
  - Hosts `/api/goose/*` REST operations for lifecycle control (run/stop/status/history).
  - Streams metrics + logs over WebSocket in sub-second cadence.
  - Persists run history, metrics, and rendered HTML/CSV/JSON reports in SQLite.
  - Integrates with the plugin registry for mistral.rs analysis and API key validation.

- **maid-apikeys**
  - Manages users (registration, activation) with Argon2 password hashing.
  - Issues JWT access tokens and hashed API keys with configurable prefixes.
  - Exposes RBAC utilities, audit logging, and credential introspection for `maid-core`.

- **maid-mistral**
  - Supplies Goose configuration recipes derived from natural-language prompts.
  - Consumes Goose metrics to produce optimisation guidance.
  - Provides multimodal endpoints (text, image, speech) and OpenAI-compatible chat completions.

## Data flows

1. Operators authenticate with maid-apikeys to obtain JWT/API keys (`write:loadtests` scope required to launch runs).
2. Dashboard posts `/api/goose/run` to maid-core, streaming live metrics via WebSocket.
3. Upon run completion, maid-core writes reports and pushes summaries to plugin bus. The mistral plugin analyses metrics via `/v1/analysis` and returns actionable insights.
4. Audit events (`test.start`, `test.stop`, `config.change`, `apikey.create`, etc.) persist within the API keys database for compliance review.

## Persistence

| Service      | Storage                       | Purpose                           |
| ------------ | ----------------------------- | --------------------------------- |
| maid-core    | `goose_runs.sqlite`           | Run history + metrics cache       |
| maid-apikeys | `apikeys.sqlite`              | Users, API keys, audit ledger     |
| maid-mistral | stateless (no DB)             | Real-time inference only          |

## Observability

- `tracing` structured logs (JSON-friendly) across all services.
- Run metrics delivered through broadcast channels allowing unlimited subscribers.
- Audit trail stored in SQLite with `audit_events` table and introspection endpoint.

## Deployment guidance

- Front the services with TLS terminators / API gateway enforcing rate limiting.
- Co-locate sqlite databases on fast disks or migrate to PostgreSQL (via SeaORM) for multi-tenant scale.
- Configure systemd units or container orchestration (Kubernetes) to ensure restart semantics and rolling upgrades.
- Enable GPU backends (CUDA/Metal) for mistral.rs when high throughput AI features are required.

