# HelixDB (BrainDB) Ecosystem

HelixDB delivers a production-ready graph + vector database core with an extensible plugin
bus for ML (pgML) and Web3 archival (DB3). This repository contains the full Rust workspace
implementing the core, CLI tooling, query macros, and plugin adapters required to operate
BrainDB in production environments.

## Workspace layout

- `crates/helix-db` – core database engine, REST API (port `6969`), HelixQL runtime, telemetry,
  plugin bus, and LMDB-backed storage for graph, document, and vector data.
- `crates/helix-cli` – operational command-line utility for configuration management, health
  checks, secure API-key provisioning, and launching the HelixDB server.
- `crates/helix-macros` – compile-time macros for authoring statically validated HelixQL
  definitions inside Rust applications.
- `crates/pgml-adapter` – plugin adapter that exposes pgML training, inference, embedding, and
  transformation capabilities through the HelixDB plugin bus.
- `crates/db3-adapter` – plugin adapter for DB3 archival, write signatures, and sync-status
  telemetry.
- `docs/` – comprehensive product documentation covering architecture, deployment, security,
  telemetry, and workflow guides.
- `braindb-config.yaml` – reference configuration aligned with production defaults.

## Getting started

```bash
cargo build
helix-cli init
helix-cli serve --config braindb-config.yaml
```

See the [deployment guide](docs/deployment.md) for secure TLS termination, production LMDB
backups, and plugin rollout procedures.
