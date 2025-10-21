# API Reference

HelixDB exposes a secure REST API for query execution, document management, telemetry, and
plugin control. All endpoints require TLS termination and API-key or JWT authentication unless
otherwise noted.

## Authentication

- **API key** – Send `X-API-Key: <key>` header. Keys are hashed with SHA3 in configuration.
- **JWT** – Send `Authorization: Bearer <token>`. Tokens are validated against configured issuer,
  audience, and RBAC scopes.

## Endpoints

### POST `/api/braindb/query`

Execute a HelixQL request.

Request body:

```json
{
  "query": "QUERY findSimilar, \"vec <- Embed(\"hello\", \"ada-002\"); docs <- VectorSearch<Document>(vec, k:10); RETURN docs;\"",
  "top_k": 5,
  "metric": "cosine"
}
```

Response:

```json
{
  "documents": [
    {
      "id": "doc-1",
      "metadata": {"source": "support"},
      "vector": [0.1, 0.2, ...]
    }
  ]
}
```

### POST `/api/braindb/documents`

Insert or upsert a document and optional embedding.

```json
{
  "id": "doc-123",
  "body": {"title": "BrainDB"},
  "metadata": {"source": "kb"},
  "embedding": [0.01, 0.02, 0.03]
}
```

The endpoint automatically indexes the vector if provided.

### GET `/api/braindb/metrics`

Returns the Prometheus metrics payload. Scope `metrics.read` is optional; if not provided the
endpoint falls back to anonymous access to support scraping.

### POST `/api/braindb/plugins/register`

Register an external plugin instance.

```json
{
  "name": "pgml",
  "base_url": "https://pgml.internal",
  "capabilities": ["pgml.train"],
  "feature_flags": ["gpu"]
}
```

## Error handling

Errors return JSON bodies with appropriate HTTP status codes:

```json
{
  "error": "authentication failure"
}
```

Refer to `docs/security.md` for authentication scope mappings and `docs/monitoring.md` for metric
names.
