# MAID Project v1.0

The MAID ecosystem orchestrates production-grade load testing, authentication, and AI-assisted analysis. It consists of three Rust microservices:

- **maid-core** – Goose-inspired load-testing engine with REST + WebSocket interfaces, reporting, and plugin bus connectivity.
- **maid-apikeys** – Security service for user management, JWT/API-key issuance, RBAC, and audit logging.
- **maid-mistral** – Lightweight mistral.rs integration that supplies AI-generated Goose configurations and post-run insights.

Each service is packaged as an independent binary within a shared Cargo workspace. Configuration is JSON-first, follows environment-variable overrides, and persists runtime state in SQLite databases under the `data/` directory by default.

## Getting started

### Prerequisites

- Rust toolchain 1.79+ (`rustup default stable`)
- SQLite 3 (bundled via `rusqlite` for portability)
- Optional: CUDA 11.8+ / Metal for running mistral.rs against GPU workloads

### Building the workspace

```bash
cargo build --release
```

### Running the services

Start each service in its own terminal (or supervise via systemd/docker):

```bash
# Authentication & RBAC service
cargo run -p maid-apikeys

# Goose load-testing core
cargo run -p maid-core

# mistral.rs AI plugin
cargo run -p maid-mistral
```

Services bind to production ports by default:

| Service        | Port  | Purpose                          |
| -------------- | ----- | -------------------------------- |
| maid-apikeys   | 43119 | Auth, API key, RBAC, audit APIs  |
| maid-core      | 43121 | Goose load controller + metrics  |
| maid-mistral   | 43140 | AI-assisted recipe + analysis    |

Override any configuration via JSON files (`config*.json`) or environment variables (`MAID__*`, `MAID_APIKEYS__*`, `MAID_MISTRAL__*`). See [docs/configuration.md](docs/configuration.md).

## Documentation suite

- [Architecture overview](docs/overview.md)
- [Goose core API reference](docs/core-api.md)
- [Authentication/API key service](docs/apikeys-api.md)
- [mistral.rs plugin guide](docs/mistral.md)
- [Configuration reference](docs/configuration.md)

The documentation covers deployment topologies, plugin bus semantics, schema-driven configuration, and dashboard integration guidance.

## Testing & quality

Automated integration tests are forthcoming; meanwhile run `cargo fmt`, `cargo clippy --workspace`, and targeted smoke runs to validate environments. Each service includes structured logging (`tracing`) and emits audit events for critical operations.

## Contributing

- Follow the security baseline: strong Argon2 password hashing, hashed API keys, JWT with configurable issuer/audience, TLS termination recommended upstream.
- Keep load testing scenarios in JSON Schema to support dashboard form generation.
- Extend plugin capabilities via the bus; register new features in `PluginRegistry` so downstream services can auto-discover.

