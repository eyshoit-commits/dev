# Architecture

HelixDB is composed of modular services that can be hot-reloaded and extended without
restarting the database node. Each component owns a focused responsibility and communicates
through typed interfaces.

## Core runtime

- **StorageEngine** – Wraps LMDB (via `heed`) with dedicated databases for nodes, edges,
  documents, and vectors. All operations are transactional and commit via write transactions.
- **GraphEngine** – Provides node/edge management, neighbor traversal, and document attachment.
- **VectorIndex** – Stores embeddings and performs cosine/L2/dot similarity search by iterating
  LMDB entries with efficient heap-based ranking.
- **HelixQlEngine** – Interprets HelixQL statements, executes deterministic embeddings, performs
  vector searches, and emits telemetry for every query.
- **AuthManager** – Manages API keys, JWT validation, and RBAC scope evaluation using SHA3
  hashing and optional issuer/audience enforcement.
- **TelemetryHub** – Buffered channel that streams `TelemetryEvent` records to the tracing system
  without blocking the request path.
- **MetricsService** – Global Prometheus recorder with optional HTTP listener bound through the
  telemetry configuration.

## REST gateway

The `HelixServer` assembles the runtime components and exposes the following routes on port
`6969`:

| Route | Method | Scope | Description |
|-------|--------|-------|-------------|
| `/api/braindb/query` | POST | `query.read` | Execute HelixQL payloads |
| `/api/braindb/documents` | POST | `documents.write` | Insert or upsert documents and optional vectors |
| `/api/braindb/metrics` | GET | `metrics.read` (optional) | Prometheus metrics snapshot |
| `/api/braindb/plugins/register` | POST | `plugins.register` | Register external plugins and capture telemetry |

CORS and request tracing are provided by `tower-http`. Authentication is enforced per-route via
API keys or JWT bearer tokens.

## Plugin bus

The plugin bus maintains a registry of capability names mapped to asynchronous handlers.
Plugins implement the `Plugin` trait and register their `Capability` implementations. Invocation
flows are routed by name and return structured payloads. Events (registration, heartbeats) are
captured as `PluginEvent` entries and exposed for audit.

## Hot reload strategy

- Configuration changes are stored in YAML and consumed on startup.
- Plugins are registered dynamically via API and can be updated without restarting the core.
- Storage directories are isolated under `data_dir/lmdb`, enabling snapshot backups and offline
  migrations.

Refer to `docs/security.md` and `docs/deployment.md` for operational guidance around protecting
and scaling this architecture.
