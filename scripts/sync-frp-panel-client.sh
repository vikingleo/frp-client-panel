#!/usr/bin/env bash

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binaries_dir="${project_root}/src-tauri/binaries"
release_version="${FRP_PANEL_VERSION:-latest}"
build_mode="${FRP_PANEL_BUILD_MODE:-source}"
# Keep release builds reproducible. Set FRP_PANEL_SOURCE_REF=latest (or a tag/commit)
# when intentionally updating the managed Client implementation.
default_source_ref="1a58b856d7de19de8669b7072872986d2fa1604a"
source_ref="${FRP_PANEL_SOURCE_REF:-${default_source_ref}}"
requested_target="${FRP_PANEL_TARGET_TRIPLE:-}"
requested_arch="${FRP_PANEL_ARCH:-}"

host_target_triple() {
  local host_os host_arch
  host_os="$(uname -s)"
  host_arch="${requested_arch:-$(uname -m)}"

  case "${host_os}/${host_arch}" in
    Darwin/arm64|Darwin/aarch64) printf '%s\n' 'aarch64-apple-darwin' ;;
    Darwin/x86_64|Darwin/amd64) printf '%s\n' 'x86_64-apple-darwin' ;;
    Linux/x86_64|Linux/amd64) printf '%s\n' 'x86_64-unknown-linux-gnu' ;;
    MINGW*/x86_64|MINGW*/amd64|MSYS*/x86_64|MSYS*/amd64|CYGWIN*/x86_64|CYGWIN*/amd64)
      printf '%s\n' 'x86_64-pc-windows-msvc'
      ;;
    *)
      printf 'Unsupported host platform: %s/%s\n' "${host_os}" "${host_arch}" >&2
      printf 'Set FRP_PANEL_TARGET_TRIPLE to one of: aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu, x86_64-pc-windows-msvc\n' >&2
      exit 1
      ;;
  esac
}

target_triple="${requested_target:-$(host_target_triple)}"

case "${target_triple}" in
  aarch64-apple-darwin)
    asset_name="frp-panel-client-darwin-arm64"
    go_os="darwin"
    go_arch="arm64"
    binary_suffix=""
    ;;
  x86_64-apple-darwin)
    asset_name="frp-panel-client-darwin-amd64"
    go_os="darwin"
    go_arch="amd64"
    binary_suffix=""
    ;;
  x86_64-unknown-linux-gnu)
    asset_name="frp-panel-client-linux-amd64"
    go_os="linux"
    go_arch="amd64"
    binary_suffix=""
    ;;
  x86_64-pc-windows-msvc)
    asset_name="frp-panel-client-windows-amd64.exe"
    go_os="windows"
    go_arch="amd64"
    binary_suffix=".exe"
    ;;
  *)
    printf 'Unsupported FRP_PANEL_TARGET_TRIPLE: %s\n' "${target_triple}" >&2
    printf 'Supported targets: aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu, x86_64-pc-windows-msvc\n' >&2
    exit 1
    ;;
esac

mkdir -p "${binaries_dir}"
temporary_file="$(mktemp "${binaries_dir}/.frp-panel-client-${target_triple}.download.XXXXXX")"
destination="${binaries_dir}/frp-panel-client-${target_triple}${binary_suffix}"
source_dir=""

cleanup() {
  rm -f "${temporary_file}"
  if [[ -n "${source_dir}" ]]; then
    rm -rf "${source_dir}"
  fi
}
trap cleanup EXIT

build_from_source() {
  if ! command -v git >/dev/null 2>&1; then
    printf 'git is required to build the frp-panel-client sidecar from source.\n' >&2
    exit 1
  fi
  if ! command -v go >/dev/null 2>&1; then
    printf 'Go 1.25+ is required to build the frp-panel-client sidecar from source.\n' >&2
    exit 1
  fi

  source_dir="$(mktemp -d "${TMPDIR:-/tmp}/frp-panel-source.XXXXXX")"
  printf 'Building frp-panel-client from upstream ref %s for %s/%s...\n' "${source_ref}" "${go_os}" "${go_arch}"
  git init --quiet "${source_dir}/source"
  git -C "${source_dir}/source" remote add origin https://github.com/VaalaCat/frp-panel.git
  git -C "${source_dir}/source" fetch --depth 1 origin "${source_ref}"
  git -C "${source_dir}/source" checkout --quiet --detach FETCH_HEAD
  (
    cd "${source_dir}/source"
    CGO_ENABLED=0 GOOS="${go_os}" GOARCH="${go_arch}" \
      go build -trimpath -buildvcs=false -o "${temporary_file}" ./cmd/frppc
  )
  printf 'Built sidecar from upstream commit: %s\n' "$(git -C "${source_dir}/source" rev-parse HEAD)"
}

download_release_asset() {
  if ! command -v curl >/dev/null 2>&1; then
    printf 'curl is required to download the frp-panel-client sidecar.\n' >&2
    exit 1
  fi

  local download_url
  if [[ "${release_version}" == "latest" ]]; then
    download_url="https://github.com/VaalaCat/frp-panel/releases/latest/download/${asset_name}"
  else
    download_url="https://github.com/VaalaCat/frp-panel/releases/download/${release_version}/${asset_name}"
  fi
  printf 'Downloading %s (%s release) for %s...\n' "${asset_name}" "${release_version}" "${target_triple}"
  curl --fail --location --proto '=https' --tlsv1.2 --retry 3 --retry-delay 1 --output "${temporary_file}" "${download_url}"
}

case "${build_mode}" in
  source)
    build_from_source
    ;;
  release)
    download_release_asset
    ;;
  *)
    printf 'Unsupported FRP_PANEL_BUILD_MODE: %s (use source or release)\n' "${build_mode}" >&2
    exit 1
    ;;
esac

if [[ ! -s "${temporary_file}" ]]; then
  printf 'Built or downloaded sidecar is empty.\n' >&2
  exit 1
fi

chmod 755 "${temporary_file}"
mv -f "${temporary_file}" "${destination}"

printf 'Installed sidecar: %s\n' "${destination}"
printf 'Tauri target triple: %s\n' "${target_triple}"
