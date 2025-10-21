# Configuration Reference

Each service reads layered configuration sources in the following order:

1. Service-specific JSON file (e.g. `config.json`, `config.apikeys.json`, `config.mistral.json`).
2. Runtime override (`config.runtime.json`, `config.apikeys.runtime.json`, etc.).
3. Environment variables:
   - `MAID__*` for maid-core (double underscore `__` maps to nested keys).
   - `MAID_APIKEYS__*` for maid-apikeys.
   - `MAID_MISTRAL__*` for maid-mistral.

Values later in the list override earlier definitions.

## maid-core

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 43121,
    "websocket_path": "/api/goose/stream"
  },
  "persistence": {
    "database_path": "data/maid/goose_runs.sqlite",
    "report_dir": "reports"
  },
  "security": {
    "require_authentication": true,
    "api_keys_url": "http://localhost:43119",
    "tls_required": false
  },
  "plugin_bus": {
    "mistral_endpoint": "http://localhost:43140",
    "api_keys_endpoint": "http://localhost:43119"
  }
}
```

## maid-apikeys

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 43119
  },
  "database": {
    "path": "data/maid/apikeys.sqlite"
  },
  "security": {
    "jwt_secret": "<strong-secret>",
    "jwt_issuer": "maid.apikeys",
    "jwt_audience": "maid.clients",
    "jwt_expiry_minutes": 60,
    "default_api_key_prefix": "maid_live_",
    "default_scopes": ["read:loadtests", "write:loadtests"]
  }
}
```

## maid-mistral

```json
{
  "host": "0.0.0.0",
  "port": 43140
}
```

## Environment overrides

Examples:

```bash
export MAID__SERVER__PORT=53121
export MAID__PLUGIN_BUS__MISTRAL_ENDPOINT="https://mistral.prod"
export MAID_APIKEYS__SECURITY__JWT_SECRET="prod-secret"
export MAID_MISTRAL__PORT=54140
```

