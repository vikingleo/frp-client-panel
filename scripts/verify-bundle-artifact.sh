#!/usr/bin/env bash

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_triple="${FRP_PANEL_TARGET_TRIPLE:-}"

if [[ -z "${target_triple}" ]]; then
  printf 'FRP_PANEL_TARGET_TRIPLE is required.\n' >&2
  exit 1
fi

bundle_root=""
for candidate in \
  "${project_root}/src-tauri/target/${target_triple}/release/bundle" \
  "${project_root}/src-tauri/target/release/bundle"; do
  if [[ -d "${candidate}" ]]; then
    bundle_root="${candidate}"
    break
  fi
done

if [[ -z "${bundle_root}" ]]; then
  printf 'No bundle directory found for target %s.\n' "${target_triple}" >&2
  exit 1
fi

first_matching_file() {
  local directory="$1"
  local name_pattern="$2"
  find "${directory}" -maxdepth 1 -type f -name "${name_pattern}" -print -quit 2>/dev/null || true
}

case "${target_triple}" in
  aarch64-apple-darwin|x86_64-apple-darwin)
    app_path="${bundle_root}/macos/frp-panel Client.app"
    dmg_path="$(first_matching_file "${bundle_root}/dmg" '*.dmg')"
    sidecar_path="${app_path}/Contents/MacOS/frp-panel-client"
    native_frpc_path="${app_path}/Contents/MacOS/frpc"

    [[ -d "${app_path}" ]] || { printf 'macOS app bundle is missing: %s\n' "${app_path}" >&2; exit 1; }
    [[ -f "${sidecar_path}" ]] || { printf 'Bundled macOS sidecar is missing: %s\n' "${sidecar_path}" >&2; exit 1; }
    [[ -f "${native_frpc_path}" ]] || { printf 'Bundled macOS frpc is missing: %s\n' "${native_frpc_path}" >&2; exit 1; }
    file "${native_frpc_path}" | grep -Eq 'Mach-O 64-bit executable (arm64|x86_64)'
    [[ -n "${dmg_path}" && -s "${dmg_path}" ]] || { printf 'macOS DMG is missing or empty.\n' >&2; exit 1; }
    codesign --verify --deep --strict "${app_path}"
    printf 'Verified macOS app, embedded sidecars, and DMG: %s\n' "${dmg_path}"
    ;;
  x86_64-unknown-linux-gnu)
    appimage_path="$(first_matching_file "${bundle_root}/appimage" '*.AppImage')"
    [[ -n "${appimage_path}" && -s "${appimage_path}" ]] || { printf 'Linux AppImage is missing or empty.\n' >&2; exit 1; }
    file "${appimage_path}" | grep -Eq 'ELF 64-bit.*x86-64'
    [[ -x "${appimage_path}" ]] || { printf 'Linux AppImage is not executable: %s\n' "${appimage_path}" >&2; exit 1; }
    appimage_extract_dir="$(mktemp -d /tmp/frp-panel-appimage.XXXXXX)"
    (
      cd "${appimage_extract_dir}"
      "${appimage_path}" --appimage-extract >/dev/null
    )
    [[ -f "${appimage_extract_dir}/squashfs-root/usr/bin/frp-panel-client" ]] || {
      printf 'Linux AppImage does not contain usr/bin/frp-panel-client.\n' >&2
      exit 1
    }
    printf 'Verified Linux AppImage and embedded sidecar: %s\n' "${appimage_path}"
    ;;
  x86_64-pc-windows-msvc)
    installer_path="$(first_matching_file "${bundle_root}/nsis" '*.exe')"
    [[ -n "${installer_path}" && -s "${installer_path}" ]] || { printf 'Windows NSIS EXE is missing or empty.\n' >&2; exit 1; }
    # NSIS may use a 32-bit bootstrapper even when it installs an x64 application.
    # The target x64 sidecar is verified before bundle creation by verify:client.
    file "${installer_path}" | grep -Eq 'PE32(\+)? executable'
    archive_tool=""
    for candidate in 7z 7zz 7za; do
      if command -v "${candidate}" >/dev/null 2>&1; then
        archive_tool="${candidate}"
        break
      fi
    done
    [[ -n "${archive_tool}" ]] || { printf '7-Zip is required to inspect the Windows NSIS payload.\n' >&2; exit 1; }
    "${archive_tool}" l "${installer_path}" 2>/dev/null | grep -qi 'frp-panel-client\.exe' || {
      printf 'Windows NSIS EXE does not contain frp-panel-client.exe.\n' >&2
      exit 1
    }
    printf 'Verified Windows NSIS EXE and embedded sidecar: %s\n' "${installer_path}"
    ;;
  *)
    printf 'Unsupported FRP_PANEL_TARGET_TRIPLE: %s\n' "${target_triple}" >&2
    exit 1
    ;;
esac
