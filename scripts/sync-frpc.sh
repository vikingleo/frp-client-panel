#!/usr/bin/env bash

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binaries_dir="${project_root}/src-tauri/binaries"
component="${FRP_COMPONENT:-frpc}"
frp_version="${FRP_VERSION:-0.71.0}"
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
      printf 'Native %s sidecar currently supports Darwin arm64 and x86_64 only.\n' "${component}" >&2
      printf 'Set FRP_PANEL_TARGET_TRIPLE explicitly when building a supported macOS target.\n' >&2
      exit 1
      ;;
  esac
}

target_triple="${requested_target:-$(host_target_triple)}"

# The source archive and its SHA-256 are intentionally pinned. To update frp,
# update this table, THIRD_PARTY_NOTICES.md, and CI verification in one change.
case "${frp_version}/${target_triple}" in
  0.71.0/aarch64-apple-darwin)
    frp_arch='arm64'
    expected_sha256='45be02b186860d375ed49a8941ae9569628a54bf14e67fc36b29c98c99dabcc6'
    ;;
  0.71.0/x86_64-apple-darwin)
    frp_arch='amd64'
    expected_sha256='1b1b4e2f1836e21e8733f1dddaacd4ed9ae67d7dbee39046b9d7b7eda6253637'
    ;;
  *)
    printf 'No audited official frp checksum is configured for version=%s target=%s.\n' \
      "${frp_version}" "${target_triple}" >&2
    exit 1
    ;;
esac

command -v curl >/dev/null 2>&1 || {
  printf 'curl is required to download the official frpc archive.\n' >&2
  exit 1
}
command -v shasum >/dev/null 2>&1 || {
  printf 'shasum is required to verify the official frpc archive.\n' >&2
  exit 1
}
command -v tar >/dev/null 2>&1 || {
  printf 'tar is required to extract the official frpc archive.\n' >&2
  exit 1
}

archive_name="frp_${frp_version}_darwin_${frp_arch}.tar.gz"
archive_url="https://github.com/fatedier/frp/releases/download/v${frp_version}/${archive_name}"
archive_member="frp_${frp_version}_darwin_${frp_arch}/${component}"
destination="${binaries_dir}/${component}-${target_triple}"
archive_path="$(mktemp "${TMPDIR:-/tmp}/${component}-${target_triple}.archive.XXXXXX")"
binary_path="$(mktemp "${binaries_dir}/.${component}-${target_triple}.extract.XXXXXX")"

cleanup() {
  rm -f "${archive_path}" "${binary_path}"
}
trap cleanup EXIT

mkdir -p "${binaries_dir}"
printf 'Downloading official %s %s for %s...\n' "${component}" "${frp_version}" "${target_triple}"
curl --fail --location --proto '=https' --tlsv1.2 --retry 3 --retry-delay 1 \
  --output "${archive_path}" "${archive_url}"

actual_sha256="$(shasum -a 256 "${archive_path}" | awk '{print $1}')"
if [[ "${actual_sha256}" != "${expected_sha256}" ]]; then
  printf 'Official frp archive checksum mismatch.\nExpected: %s\nActual:   %s\n' \
    "${expected_sha256}" "${actual_sha256}" >&2
  exit 1
fi

tar -xOf "${archive_path}" "${archive_member}" > "${binary_path}"
if [[ ! -s "${binary_path}" ]]; then
  printf 'Extracted %s sidecar is empty.\n' "${component}" >&2
  exit 1
fi

chmod 755 "${binary_path}"
mv -f "${binary_path}" "${destination}"

printf 'Installed verified official %s sidecar: %s\n' "${component}" "${destination}"
printf 'Archive SHA-256: %s\n' "${actual_sha256}"
