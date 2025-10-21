# BrainDB Ecosystem Overview

BrainDB unifies graph, document, and vector workloads behind HelixDB, a Rust-based data
platform engineered for sub-10ms responses, hybrid search, and secure plugin integration.
This document provides a high-level summary of the ecosystem and the responsibilities of each
workspace component.

## Core capabilities

- **Unified data model** – Nodes, edges, documents, and vector embeddings are backed by an
  LMDB storage engine with transactional guarantees and hot-reload support.
- **HelixQL** – Declarative query language with compile-time validation through the
  `helix_query!` macro and runtime execution via the HelixQL engine.
- **Hybrid search** – Embed text with deterministic hashing, store vectors, and run cosine/L2/dot
  similarity lookups.
- **REST gateway** – Hardened API on port `6969` serving `/api/braindb/query`, `/documents`,
  `/metrics`, and `/plugins/register` with API-key and JWT enforcement.
- **Telemetry** – Prometheus exporter, structured tracing, and asynchronous telemetry events for
  queries, vector searches, document inserts, and plugin registrations.

## Plugin ecosystem

| Plugin | Purpose | Capabilities |
|--------|---------|--------------|
| `pgml-adapter` | In-database ML via ParadisML/pgML | `train`, `predict`, `embed`, `transform` |
| `db3-adapter` | Web3 archival and sync telemetry | `add_doc`, `query`, `archive_status` |

Each adapter implements the shared `Plugin` and `Capability` traits and registers with the
HelixDB plugin bus for runtime dispatch.

## CLI tooling

`helix-cli` orchestrates configuration initialization (`helix init`), validation (`helix
check`), deployment simulation (`helix push dev`), secure API-key provisioning, and launching the
HelixDB server.

Refer to the remaining documentation for deep dives into architecture, deployment, and security
practices.
