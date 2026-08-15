#!/usr/bin/env bash

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
requested_target="${FRP_PANEL_TARGET_TRIPLE:-}"

host_target_triple() {
  case "$(uname -s)/$(uname -m)" in
    Darwin/arm64|Darwin/aarch64) printf '%s\n' 'aarch64-apple-darwin' ;;
    Darwin/x86_64|Darwin/amd64) printf '%s\n' 'x86_64-apple-darwin' ;;
    Linux/x86_64|Linux/amd64) printf '%s\n' 'x86_64-unknown-linux-gnu' ;;
    MINGW*/x86_64|MINGW*/amd64|MSYS*/x86_64|MSYS*/amd64|CYGWIN*/x86_64|CYGWIN*/amd64)
      printf '%s\n' 'x86_64-pc-windows-msvc'
      ;;
    *)
      printf 'Unsupported host platform: %s/%s\n' "$(uname -s)" "$(uname -m)" >&2
      exit 1
      ;;
  esac
}

target_triple="${requested_target:-$(host_target_triple)}"

case "${target_triple}" in
  aarch64-apple-darwin)
    expected_format='Mach-O.*arm64'
    binary_suffix=""
    ;;
  x86_64-apple-darwin)
    expected_format='Mach-O.*x86_64'
    binary_suffix=""
    ;;
  x86_64-unknown-linux-gnu)
    expected_format='ELF 64-bit.*x86-64'
    binary_suffix=""
    ;;
  x86_64-pc-windows-msvc)
    expected_format='PE32.*x86-64'
    binary_suffix=".exe"
    ;;
  *)
    printf 'Unsupported FRP_PANEL_TARGET_TRIPLE: %s\n' "${target_triple}" >&2
    exit 1
    ;;
esac

binary="${project_root}/src-tauri/binaries/frp-panel-client-${target_triple}${binary_suffix}"
if [[ ! -f "${binary}" ]]; then
  printf 'Sidecar not found: %s\n' "${binary}" >&2
  printf 'Run FRP_PANEL_TARGET_TRIPLE=%s pnpm sync:client first.\n' "${target_triple}" >&2
  exit 1
fi

if [[ ! -x "${binary}" ]]; then
  printf 'Sidecar is not executable: %s\n' "${binary}" >&2
  exit 1
fi

if ! command -v file >/dev/null 2>&1; then
  printf 'file is required to inspect the sidecar.\n' >&2
  exit 1
fi

description="$(file "${binary}")"
if ! grep -Eq "${expected_format}" <<<"${description}"; then
  printf 'Sidecar format or architecture mismatch. Expected /%s/, got: %s\n' "${expected_format}" "${description}" >&2
  exit 1
fi

if [[ "${target_triple}" == "$(host_target_triple)" ]]; then
  if ! "${binary}" client --help >/dev/null 2>&1; then
    printf 'Sidecar CLI smoke test failed: %s client --help\n' "${binary}" >&2
    exit 1
  fi
  printf 'CLI smoke test: passed\n'
else
  printf 'CLI smoke test: skipped (cross-target %s on %s)\n' "${target_triple}" "$(host_target_triple)"
fi

printf 'Verified sidecar: %s\n' "${binary}"
printf 'Target: %s\n' "${target_triple}"
printf 'Binary: %s\n' "${description}"
