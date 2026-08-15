# Third-party notices

This project contains original desktop-client code and distributes a separately
executed `frp-panel-client` sidecar. The sidecar is built from the upstream
source project listed below.

## VaalaCat/frp-panel

- Component: `frp-panel-client` sidecar.
- Source repository: `VaalaCat/frp-panel`.
- Pinned source revision: `1a58b856d7de19de8669b7072872986d2fa1604a`.
- Build entry point: `./cmd/frppc`.
- Build instructions: `scripts/sync-frp-panel-client.sh`.
- License: GNU Affero General Public License, version 3.
- Local modifications: none. This repository compiles the pinned upstream
  revision with `CGO_ENABLED=0` for each supported target; it does not patch the
  upstream source.

The corresponding source code can be obtained by cloning the upstream
repository and checking out the pinned revision above. The same build command
is documented in `scripts/sync-frp-panel-client.sh` and `docs/DEVELOPMENT.md`.

## Tauri, Vue, Rust crates, npm packages, and system libraries

The desktop shell depends on Tauri, Vue, Rust crates, npm packages, and native
system libraries. Their exact versions and licenses are recorded in
`src-tauri/Cargo.lock` and `pnpm-lock.yaml`. Release automation should generate
an SPDX or CycloneDX SBOM so each published artifact carries a complete,
machine-readable dependency inventory.

## Trademark and affiliation notice

`frp-panel` and `frp-panel-client` identify the upstream project. This project
is an independent community desktop client and is not affiliated with, endorsed
by, or supported by the upstream maintainers unless they explicitly state so.
