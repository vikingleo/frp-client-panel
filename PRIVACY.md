# Privacy and local-data policy

## Scope

This policy describes the desktop application in this repository. It does not
replace the privacy policy of the `frp-panel` server you connect to.

## Data stored locally

| Data | Storage | Purpose |
| --- | --- | --- |
| Client ID, API URL, RPC URL, startup preferences, TLS exception preference | Application configuration store | Reconnect and launch behavior |
| Client Secret | System credential store (Keychain, Credential Manager, or Secret Service) | Authenticate the managed client |
| Native `frpc.toml` / `frps.toml` | App-private configuration directory; Unix files use user-private permissions | Run native Client or local Server Profiles |
| `frps` Dashboard credentials | Stored in an App-managed `frps.toml`; read while refreshing the local Dashboard | Authenticate read-only local status requests; not returned by the status API or persisted in Profile JSON. The TOML is visible in the local advanced editor only when the user edits that managed Profile. |
| Runtime logs | In-memory ring buffer for the current application session | Status and troubleshooting |

The Client Secret is not written to the application configuration JSON. It is
passed to the managed sidecar through that child process's environment instead
of its command-line arguments.

## Network activity

The application starts the upstream `frp-panel-client` sidecar. That process
connects only to the API and RPC endpoints you configure, subject to your
network, proxy, and server configuration. In native `frpc` mode it connects to
the `frps` address in the managed TOML. In local `frps` mode it accepts
connections according to the managed server TOML. When Dashboard is enabled,
the Rust backend makes read-only requests to that configured Dashboard endpoint;
it does not send data to a project telemetry service or issue commands to
remote clients. The desktop shell does not include analytics, advertising SDKs,
telemetry uploads, or automatic crash-report uploading.

## TLS behavior

TLS certificate verification is enabled by default for panel/API/RPC
connections. A user can explicitly allow an insecure TLS exception for a
self-signed panel deployment; doing so weakens transport security and can
expose the connection to a man-in-the-middle attack. The local Dashboard reader
does not disable HTTPS certificate verification and does not support an
insecure Dashboard exception.

## Diagnostics and support

Before attaching logs or configuration excerpts to an issue, remove Client IDs,
hostnames, addresses, tokens, Dashboard passwords, and any sensitive operational
details. The application masks managed Client Secrets and common server secret
lines in displayed logs, but users remain responsible for reviewing what they
share.
