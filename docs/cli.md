# helix-cli Reference

`helix-cli` manages configuration, lifecycle operations, and secure API keys for HelixDB.

## Commands

| Command | Description |
|---------|-------------|
| `helix-cli init` | Generate `braindb-config.yaml` with production defaults. |
| `helix-cli serve --config <path>` | Launch the HelixDB server using the specified configuration. |
| `helix-cli check --config <path>` | Validate configuration files without starting the server. |
| `helix-cli push <env>` | Simulate deployment for the target environment (default `dev`). |
| `helix-cli apikey create --name <name> --scopes scope1,scope2` | Generate a new API key and persist its hash into configuration. |

## API key workflow

1. Run `helix-cli apikey create --name ops --scopes query.read,documents.write`.
2. Store the printed secret in a password manager or vault.
3. Distribute only to trusted operators and rotate quarterly.

## Environment variables

- `RUST_LOG` – controls log verbosity (`RUST_LOG=helix_db=debug,helix_cli=info`).
- `HELIX_*` – override configuration keys (e.g. `HELIX_REST__BIND_ADDR=0.0.0.0:6969`).

## Troubleshooting

- `Configuration already exists` – remove or back up the existing YAML before re-running `init`.
- `authentication failure` – verify API key scopes or JWT issuer/audience settings.
- `storage error` – ensure the configured LMDB directory is writable by the HelixDB process.
