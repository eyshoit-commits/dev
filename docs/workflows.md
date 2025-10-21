# Reference Workflows

This document describes the primary BrainDB workflows: Retrieval-Augmented Generation (RAG) and
the training-to-production pipeline.

## RAG pipeline

1. **Embedding** – `pgml-adapter` capability `pgml.embed` embeds documents or ad-hoc prompts.
2. **Vector index** – HelixDB stores embeddings via `VectorIndex::upsert_vector`.
3. **Hybrid search** – Execute HelixQL queries combining `Embed` and `VectorSearch<Document>`.
4. **Context assembly** – Application layer stitches returned documents into LLM prompts.
5. **Archival (optional)** – Persist results or source documents via `db3-adapter` `add_doc`.

## Training to production

1. **Training** – Invoke `pgml.train` capability with training parameters stored in HelixDB.
2. **Evaluation** – Collect metrics via pgML dashboard and HelixDB telemetry events.
3. **Promotion** – Update HelixDB collections with model metadata and mark as production-ready.
4. **Archival** – Push training artifacts to DB3 using `db3.archive_status` for immutable audit.
5. **Monitoring** – Track inference latency with Prometheus and telemetry event streams.

## Automation tips

- Script workflows using the HelixDB REST API combined with plugin capabilities.
- Store prompts, contexts, and outcomes as documents to enable audit and replay.
- Schedule DB3 archival for nightly snapshots to guarantee compliance retention.
