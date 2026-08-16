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

## fatedier/frp

- Component: official `frpc` and `frps` sidecars for native FRP Client and Server Profiles.
- Source repository: `fatedier/frp`.
- Pinned version: `v0.71.0`.
- macOS source assets:
  - `frp_0.71.0_darwin_arm64.tar.gz`
    - SHA-256: `45be02b186860d375ed49a8941ae9569628a54bf14e67fc36b29c98c99dabcc6`
  - `frp_0.71.0_darwin_amd64.tar.gz`
    - SHA-256: `1b1b4e2f1836e21e8733f1dddaacd4ed9ae67d7dbee39046b9d7b7eda6253637`
- Acquisition and verification: `scripts/sync-frpc.sh` and `scripts/sync-frps.sh`
  download a pinned official archive, verify the archive SHA-256 before extraction,
  and copy only the matching `frpc` or `frps` executable into the Tauri sidecar directory.
- License: Apache License 2.0; the official source archive contains the
  corresponding license text.
- Local modifications: none.

## Tauri, Vue, Rust crates, npm packages, and system libraries

The desktop shell depends on Tauri, Vue, Rust crates, npm packages, and native
system libraries. Their exact versions and licenses are recorded in
`src-tauri/Cargo.lock` and `pnpm-lock.yaml`. Release automation should generate
an SPDX or CycloneDX SBOM so each published artifact carries a complete,
machine-readable dependency inventory.

## Trademark and affiliation notice

`frp`, `frp-panel`, and `frp-panel-client` identify their respective upstream
projects. This project is an independent community desktop client and is not
affiliated with, endorsed by, or supported by the upstream maintainers unless
they explicitly state so.
