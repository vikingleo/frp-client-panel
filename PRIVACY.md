# Privacy and local-data policy

## Scope

This policy describes the desktop application in this repository. It does not
replace the privacy policy of the `frp-panel` server you connect to.

## Data stored locally

| Data | Storage | Purpose |
| --- | --- | --- |
| Client ID, API URL, RPC URL, startup preferences, TLS exception preference | Application configuration store | Reconnect and launch behavior |
| Client Secret | System credential store (Keychain, Credential Manager, or Secret Service) | Authenticate the managed client |
| Runtime logs | In-memory ring buffer for the current application session | Status and troubleshooting |

The Client Secret is not written to the application configuration JSON. It is
passed to the managed sidecar through that child process's environment instead
of its command-line arguments.

## Network activity

The application starts the upstream `frp-panel-client` sidecar. That process
connects only to the API and RPC endpoints you configure, subject to your
network, proxy, and server configuration. The desktop shell does not include
analytics, advertising SDKs, telemetry uploads, or automatic crash-report
uploading.

## TLS behavior

TLS certificate verification is enabled by default. A user can explicitly
allow an insecure TLS exception for self-signed deployments; doing so weakens
transport security and can expose the connection to a man-in-the-middle attack.

## Diagnostics and support

Before attaching logs to an issue, remove client IDs, hostnames, addresses, and
any sensitive operational details. The application masks the currently loaded
Client Secret in displayed logs, but users remain responsible for reviewing
what they share.
