# Deployment Guide

This guide explains how to deploy HelixDB (BrainDB) with production-grade defaults and plugin
integrations.

## Prerequisites

- Rust toolchain (1.76+) for building the workspace.
- LMDB-friendly filesystem with SSD-backed storage for low-latency access.
- Kubernetes or bare-metal hosts with systemd for process supervision.
- Prometheus-compatible monitoring stack.

## Build and package

```bash
cargo build --release
```

Copy the resulting binaries (`helix-cli`) to your deployment nodes alongside the `braindb-config.yaml`
file customized for the environment.

## Configuration

1. Run `helix-cli init` to scaffold a local configuration.
2. Edit `braindb-config.yaml` to provide:
   - `data_dir` pointing to a dedicated volume.
   - `rest.bind_addr` (default `0.0.0.0:6969`).
   - `security.api_keys` (64-character SHA3-hashed values).
   - `telemetry.prometheus_endpoint` for metrics scraping (e.g. `0.0.0.0:9600`).
   - `plugins` entries pointing to pgML and DB3 services.
3. Commit the file to secure configuration management (Vault, SOPS, or sealed secrets).

## Launching the server

```bash
helix-cli serve --config /etc/helix/braindb-config.yaml
```

Recommended systemd unit snippet:

```
[Service]
ExecStart=/usr/local/bin/helix-cli serve --config /etc/helix/braindb-config.yaml
Restart=on-failure
LimitNOFILE=131072
Environment=RUST_LOG=info
```

## Plugin rollout

1. Deploy pgML (`ghcr.io/postgresml/postgresml:2.7.12`) and expose SQL + REST endpoints.
2. Deploy DB3 (`ghcr.io/dbpunk-labs/db3:latest`) with configured `ADMIN_ADDR` and `ROLLUP_INTERVAL`.
3. Register plugins with HelixDB:

```bash
curl -X POST https://helix.example.com/api/braindb/plugins/register \
  -H "X-API-Key: <admin-key>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "pgml",
    "base_url": "https://pgml.internal",
    "capabilities": ["pgml.train", "pgml.predict", "pgml.embed", "pgml.transform"],
    "feature_flags": ["gpu", "pgvector"]
  }'
```

Repeat for DB3 with the `db3.add_doc`, `db3.query`, and `db3.archive_status` capabilities.

## Backup and recovery

- Take LMDB snapshots by stopping writes, copying the `data_dir/lmdb` directory, and resuming the
  service.
- Archive configuration and API-key manifests in secure storage.
- Use DB3 to store immutable audit trails for compliance workloads.

## Upgrades

1. Build new binaries with `cargo build --release`.
2. Deploy using blue/green or rolling restart strategies.
3. Plugins can be refreshed independently via the registration endpoint without restarting the
   core service.
