#!/usr/bin/env bash
# Build the agent (`seagent`, root workspace) for a target triple and put it where Tauri expects
# the sidecar: apps/desktop/src-tauri/binaries/seagent-<triple>[.exe] (bundle.externalBin).
#
#   apps/desktop/scripts/prepare-sidecar.sh                         # host triple
#   apps/desktop/scripts/prepare-sidecar.sh x86_64-pc-windows-msvc
#   apps/desktop/scripts/prepare-sidecar.sh universal-apple-darwin  # both macOS arches + lipo
#   SEAGENT_BIN=/path/to/seagent apps/desktop/scripts/prepare-sidecar.sh   # use a prebuilt binary
#
# Desktop triples need the built-in VM staged first (scripts/prepare-vm.sh <triple>); the image
# pin is taken from src-tauri/vm/bundle.json. SEAGENT_NO_VM_BUNDLE=1 builds without the VM.
#
# PROFILE=debug for a quicker local build. Android does not use this (no sidecars on mobile):
# see scripts/prepare-android.sh.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
out="$root/apps/desktop/src-tauri/binaries"
host="$(rustc -vV | sed -n 's/^host: //p')"
triple="${1:-$host}"
profile="${PROFILE:-release}"
mkdir -p "$out"

ext=""
case "$triple" in *windows*) ext=".exe" ;; esac

# Desktop apps ship the built-in VM (resources `vm/`, staged by scripts/prepare-vm.sh) and the
# agent must pin its image (SEAGENT_VM_IMAGE_MANIFEST_SHA256, read here from vm/bundle.json).
# Without it Tauri silently bundles no VM and the tools fall back to the OS sandbox (#15), so stop
# unless the build says it means that: SEAGENT_NO_VM_BUNDLE=1.
vm_os=""
case "$triple" in *windows*) vm_os=windows ;; *apple-darwin*) vm_os=macos ;; *linux-gnu*|*linux-musl*) vm_os=linux ;; esac
bundle="$root/apps/desktop/src-tauri/vm/bundle.json"
if [[ -n "$vm_os" && "${SEAGENT_NO_VM_BUNDLE:-}" == 1 ]]; then
  echo "prepare-sidecar.sh: SEAGENT_NO_VM_BUNDLE=1 — this app has NO built-in VM (tools use the OS sandbox)" >&2
elif [[ -n "$vm_os" ]]; then
  if [[ ! -f "$bundle" ]]; then
    cat >&2 <<MSG
prepare-sidecar.sh: apps/desktop/src-tauri/vm/bundle.json is missing, so the app would ship without
its built-in VM. Stage it first (builds the guest image — in a Linux container on macOS/Windows —
and fetches the $vm_os hypervisor):

    apps/desktop/scripts/prepare-vm.sh $triple

or build without the VM on purpose: SEAGENT_NO_VM_BUNDLE=1 $0 $*
MSG
    exit 1
  fi
  b_os="$(sed -n 's/^  "os": *"\([a-z]*\)".*/\1/p' "$bundle" | head -1)"
  b_sha="$(grep -o '"manifest_sha256": *"[0-9a-f]\{64\}"' "$bundle" | grep -o '[0-9a-f]\{64\}' | head -1)"
  if [[ "$b_os" != "$vm_os" ]]; then
    echo "prepare-sidecar.sh: src-tauri/vm is staged for '$b_os', not $vm_os ($triple): run apps/desktop/scripts/prepare-vm.sh $triple" >&2
    exit 1
  fi
  if [[ -z "$b_sha" ]]; then
    echo "prepare-sidecar.sh: $bundle names no image manifest_sha256; re-stage with prepare-vm.sh" >&2
    exit 1
  fi
  if [[ -n "${SEAGENT_VM_IMAGE_MANIFEST_SHA256:-}" && "$SEAGENT_VM_IMAGE_MANIFEST_SHA256" != "$b_sha" ]]; then
    echo "prepare-sidecar.sh: SEAGENT_VM_IMAGE_MANIFEST_SHA256=$SEAGENT_VM_IMAGE_MANIFEST_SHA256 but src-tauri/vm holds image $b_sha" >&2
    exit 1
  fi
  export SEAGENT_VM_IMAGE_MANIFEST_SHA256="$b_sha"
  echo "prepare-sidecar.sh: built-in VM for $vm_os staged; the agent pins image $b_sha" >&2
  if [[ -n "${SEAGENT_BIN:-}" ]]; then
    echo "prepare-sidecar.sh: note: SEAGENT_BIN is prebuilt — it must have been built with SEAGENT_VM_IMAGE_MANIFEST_SHA256=$b_sha" >&2
  fi
fi

build() {
  local t="$1"
  local flag="--profile $profile"; [[ "$profile" == debug ]] && flag=""
  (cd "$root" && cargo build $flag --locked -p seagent --bin seagent --target "$t") >&2
  echo "$root/target/$t/$profile/seagent$ext"
}

if [[ -n "${SEAGENT_BIN:-}" ]]; then
  cp "$SEAGENT_BIN" "$out/seagent-$triple$ext"
elif [[ "$triple" == universal-apple-darwin ]]; then
  a="$(build aarch64-apple-darwin)"
  x="$(build x86_64-apple-darwin)"
  lipo -create -output "$out/seagent-universal-apple-darwin" "$a" "$x"
  # `cargo tauri build --target universal-apple-darwin` also looks for the per-arch names.
  cp "$a" "$out/seagent-aarch64-apple-darwin"
  cp "$x" "$out/seagent-x86_64-apple-darwin"
else
  cp "$(build "$triple")" "$out/seagent-$triple$ext"
fi
chmod +x "$out"/seagent-* 2>/dev/null || true
# The Linux AppImage bundles a CJK font (bundle.linux.appimage.files): fetch it (pinned + sha256).
case "$triple" in *linux-gnu*) "$here/fetch-cjk-font.sh" ;; esac
# The Windows installers bundle the virtual display driver (resources vdd/, pinned + sha256).
case "$triple" in *windows*) "$here/fetch-vdd.sh" ;; esac
ls -l "$out"
