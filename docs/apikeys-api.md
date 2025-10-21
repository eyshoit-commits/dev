# maid-apikeys Service

Base URL: `http://<host>:43119`

The service exposes REST endpoints for user lifecycle, authentication, API key governance, and audit logging. Data persists to `apikeys.sqlite` with referential integrity enforced.

## Authentication flows

### Register

`POST /api/auth/register`

```json
{
  "email": "ops@example.com",
  "password": "S3cure!Passphrase",
  "display_name": "Ops Engineer",
  "scopes": ["read:loadtests", "write:loadtests"],
  "roles": ["loadtest.admin"]
}
```

### Login

`POST /api/auth/login`

Returns a signed JWT with scopes merged from defaults and user-specific assignments.

```json
{
  "token": "<jwt>",
  "expires_at": "2024-06-15T13:00:00Z",
  "scopes": ["read:loadtests", "write:loadtests"],
  "user": {
    "id": "b0c3...",
    "email": "ops@example.com",
    "display_name": "Ops Engineer"
  }
}
```

### Me / Logout

- `GET /api/auth/me` – Returns the authenticated profile.
- `POST /api/auth/logout` – Audits logout events (stateless token invalidation assumed at gateway).

### Introspect

`POST /api/auth/introspect`

```json
{
  "credential": "maid_live_00a1...",
  "kind": "api_key"
}
```

Response indicates whether the credential is active along with authorised scopes. The Goose core uses this endpoint to validate API keys and JWTs prior to executing privileged operations.

## API key management

### Create

`POST /api/apikeys`

```json
{
  "name": "dashboard-automation",
  "scopes": ["read:loadtests"],
  "prefix": "maid_live_",
  "expires_at": "2024-06-30T00:00:00Z"
}
```

Response returns the plaintext key once. Store it securely; the database only retains SHA-256 hashes.

### List / revoke / rotate

- `GET /api/apikeys` – List keys for the current user.
- `DELETE /api/apikeys/{id}` – Immediate revocation.
- `POST /api/apikeys/{id}/rotate` – Issue a new secret while preserving metadata.

## RBAC & scopes

Scopes follow the `{action}:{resource}` convention. Default scopes:

- `read:loadtests`
- `write:loadtests`

Attach additional scopes (e.g., `read:auth.audit`) to extend functionality. Roles are stored as free-form strings to support hierarchical interpretation at the dashboard layer.

## Audit events

Every security-sensitive change records an `audit_events` row with timestamp, actor, and JSON payload. Query the table directly or extend the service with `GET /api/audit` (roadmap) for compliance exports.

