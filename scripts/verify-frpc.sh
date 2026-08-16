#!/usr/bin/env bash

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
requested_target="${FRP_PANEL_TARGET_TRIPLE:-}"
requested_arch="${FRP_PANEL_ARCH:-}"

host_target_triple() {
  local host_os host_arch
  host_os="$(uname -s)"
  host_arch="${requested_arch:-$(uname -m)}"
  case "${host_os}/${host_arch}" in
    Darwin/arm64|Darwin/aarch64) printf '%s\n' 'aarch64-apple-darwin' ;;
    Darwin/x86_64|Darwin/amd64) printf '%s\n' 'x86_64-apple-darwin' ;;
    *)
      printf 'Cannot infer a supported Darwin target from %s/%s.\n' "${host_os}" "${host_arch}" >&2
      exit 1
      ;;
  esac
}

actual_host_target_triple() {
  case "$(uname -s)/$(uname -m)" in
    Darwin/arm64|Darwin/aarch64) printf '%s\n' 'aarch64-apple-darwin' ;;
    Darwin/x86_64|Darwin/amd64) printf '%s\n' 'x86_64-apple-darwin' ;;
    *) printf '%s\n' 'unsupported' ;;
  esac
}

target_triple="${requested_target:-$(host_target_triple)}"
case "${target_triple}" in
  aarch64-apple-darwin) expected_arch='arm64' ;;
  x86_64-apple-darwin) expected_arch='x86_64' ;;
  *)
    printf 'Native frpc verification only supports Darwin arm64 and x86_64 targets.\n' >&2
    exit 1
    ;;
esac

binary_path="${project_root}/src-tauri/binaries/frpc-${target_triple}"
[[ -f "${binary_path}" ]] || {
  printf 'Official frpc sidecar is missing: %s\n' "${binary_path}" >&2
  exit 1
}
[[ -x "${binary_path}" ]] || {
  printf 'Official frpc sidecar is not executable: %s\n' "${binary_path}" >&2
  exit 1
}

file_description="$(file "${binary_path}")"
if [[ "${file_description}" != *"Mach-O"* || "${file_description}" != *"${expected_arch}"* ]]; then
  printf 'Unexpected frpc binary format for %s: %s\n' "${target_triple}" "${file_description}" >&2
  exit 1
fi

host_target="$(actual_host_target_triple)"
if [[ "${host_target}" == "${target_triple}" ]]; then
  version_output="$("${binary_path}" --version 2>&1)"
  [[ -n "${version_output}" ]] || {
    printf 'frpc --version returned no output.\n' >&2
    exit 1
  }
  printf '%s\n' "${version_output}"

  fixture_directory="${project_root}/src-tauri/tests/fixtures"
  for fixture_name in \
    frpc-valid.toml \
    frpc-generated-tcp.toml \
    frpc-generated-udp.toml \
    frpc-generated-http.toml \
    frpc-generated-https.toml; do
    fixture_path="${fixture_directory}/${fixture_name}"
    [[ -f "${fixture_path}" ]] || {
      printf 'Native frpc fixture is missing: %s\n' "${fixture_path}" >&2
      exit 1
    }
    "${binary_path}" verify -c "${fixture_path}"
  done
else
  printf 'Cross-target binary validated without execution: %s\n' "${target_triple}"
fi

printf 'Verified official frpc sidecar: %s\n' "${binary_path}"
