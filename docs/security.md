# Security Guide

HelixDB is private-by-default and enforces strict authentication, authorization, and telemetry
controls.

## Authentication

- **API Keys**: Configure hashed keys in `braindb-config.yaml`. Use `helix-cli apikey create` to
  generate 64-character secrets stored securely outside the repository.
- **JWT**: Configure issuer and audience claims; the `AuthManager` validates scopes and roles.
- **RBAC**: Map roles to scopes under `security.rbac_roles` in the configuration file.

## Required scopes

| Scope | Description |
|-------|-------------|
| `query.read` | Execute HelixQL queries |
| `documents.write` | Insert documents or embeddings |
| `metrics.read` | Access Prometheus metrics (optional) |
| `plugins.register` | Register or update plugin endpoints |

## Transport security

- Terminate TLS in front of HelixDB using Envoy, NGINX, or Kubernetes ingress.
- Restrict the REST listener to private networks when possible.
- Use mutual TLS between HelixDB and pgML/DB3 services or secure service mesh policies.

## Secrets management

- Store API keys, JWT signing secrets, and DB3 signing keys in a vault (HashiCorp Vault, AWS KMS).
- Rotate secrets regularly and leverage `helix-cli apikey create` for fast rollout.

## Auditing

- Telemetry events record plugin registrations, vector searches, and document inserts.
- Export events to a SIEM by tailing structured logs or connecting the telemetry hub to Kafka.
- Use DB3 archival for immutable audit trails of query responses and training jobs.

## Hardening checklist

- Enable rate limiting on ingress.
- Enforce IP allow-lists for administrative routes (`/plugins/register`).
- Keep dependencies updated (`cargo update`) and run SAST scanners on the workspace.
- Regularly back up LMDB and store copies off-site with encryption-at-rest.
