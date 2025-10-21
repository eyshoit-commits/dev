# maid-mistral Plugin Guide

The mistral.rs plugin offers AI-assisted configuration generation, metrics analysis, and OpenAI-compatible chat completions. It is stateless and binds to `43140` by default.

## Endpoints

| Endpoint                  | Description                                              |
| ------------------------- | -------------------------------------------------------- |
| `POST /v1/recipes`        | Convert natural-language prompts into Goose JSON config. |
| `POST /v1/analysis`       | Analyse Goose metrics array and emit bottleneck report.  |
| `POST /api/inference/text`| Text → Text inference for operator tooling.              |
| `POST /api/inference/image`| Generate illustrative diagrams/heatmaps metadata.       |
| `POST /api/inference/speech`| Text → Speech links (Dia 1.6B voice).                  |
| `POST /v1/chat/completions`| OpenAI-compatible completion endpoint.                  |

## Recipe generation

Request:

```json
{
  "model": "gpt-4o-mini",
  "input": "Load test checkout flow hitting https://shop.example 200 users"
}
```

Response includes a schema-compliant Goose configuration. The service heuristically extracts base URLs, user counts, and scenario hints.

## Analysis workflow

Pass the metrics array (`StreamEnvelope::Metrics` snapshots) to `/v1/analysis`. The plugin computes aggregate throughput, error rate, and latency percentiles to produce actionable recommendations, e.g. scaling hints or retry advisories.

## Deployment

- Stateless runtime – scale horizontally behind a load balancer.
- Configure GPU/CPU acceleration by chaining to native mistral.rs for heavy inference if desired (current implementation provides deterministic heuristics for offline demos).
- Authenticate upstream clients at API gateway level; optionally wire through maid-apikeys introspection if extended.

