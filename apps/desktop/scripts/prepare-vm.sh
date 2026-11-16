#!/usr/bin/env bash
# Stage the built-in VM for a local desktop build — the step CI runs before prepare-sidecar.sh
# (.github/workflows/desktop.yml: `vm-image` job + packaging/vm/bundle.sh). Without it the app has
# no microVM and its tools fall back to the OS sandbox, so prepare-sidecar.sh and the release
# build refuse to continue until apps/desktop/src-tauri/vm/bundle.json exists.
#
#   apps/desktop/scripts/prepare-vm.sh                          # for the host's desktop triple
#   apps/desktop/scripts/prepare-vm.sh universal-apple-darwin   # macOS (aarch64 guest, vfkit)
#   apps/desktop/scripts/prepare-vm.sh x86_64-pc-windows-msvc   # Windows (x86_64 guest, QEMU)
#   apps/desktop/scripts/prepare-vm.sh x86_64-unknown-linux-gnu # Linux (x86_64 guest, Firecracker)
#   IMAGE=/path/to/vm-image apps/desktop/scripts/prepare-vm.sh … # reuse a built image (e.g. CI's
#                                                                # `vm-bundle-image-<arch>` artifact)
#
# The guest image (a Linux kernel built from source + a static musl guest agent) is built by
# packaging/vm/build-image.sh, which needs Linux. On a Linux host with the guest's CPU it runs
# directly (packages: build-essential flex bison bc libelf-dev cpio xz-utils musl-tools, and the
# Rust target <arch>-unknown-linux-musl). On macOS and Windows — or for the other CPU — it runs in
# a Linux container (Docker or Podman; CONTAINER=podman to choose): an arm64 container on Apple
# Silicon, amd64 for Windows. The image is then staged with the target OS's hypervisor
# (packaging/vm/bundle.sh, pinned hashes) into apps/desktop/src-tauri/vm, whose bundle.json carries
# the manifest sha256 that prepare-sidecar.sh compiles into the agent.
#
# Env: ARCH (guest CPU, default per OS: macOS aarch64, else the triple's CPU), IMAGE, CONTAINER,
# CACHE (bundle.sh download cache).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
host="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
triple="${1:-$host}"
[[ -n "$triple" ]] || { echo "prepare-vm.sh: no target triple (and rustc is not on PATH)" >&2; exit 2; }

case "$triple" in
  *windows*) os=windows; arch="${ARCH:-x86_64}" ;;
  *apple-darwin*) os=macos; arch="${ARCH:-aarch64}" ;;
  aarch64*linux*) os=linux; arch="${ARCH:-aarch64}" ;;
  *linux*) os=linux; arch="${ARCH:-x86_64}" ;;
  *) echo "prepare-vm.sh: $triple has no built-in VM (Android and iOS apps ship without one)" >&2; exit 2 ;;
esac

img="${IMAGE:-$root/target/vm-image-$arch}"
if [[ -n "${IMAGE:-}" ]]; then
  echo "prepare-vm.sh: using the image in $img"
elif [[ "$(uname -s)" == Linux && "$(uname -m)" == "$arch" ]]; then
  echo "prepare-vm.sh: building the $arch guest image on this host → $img" >&2
  export "CC_${arch}_unknown_linux_musl=${CC_MUSL:-musl-gcc}"
  "$root/packaging/vm/build-image.sh" --out "$img" --arch "$arch" --build-kernel
else
  engine="${CONTAINER:-}"
  if [[ -z "$engine" ]]; then
    for e in docker podman; do command -v "$e" >/dev/null 2>&1 && { engine="$e"; break; }; done
  fi
  if [[ -z "$engine" ]]; then
    cat >&2 <<EOF
prepare-vm.sh: the guest image needs Linux to build (a kernel and a musl guest agent), and neither
Docker nor Podman is installed. Either install one (Docker Desktop, OrbStack, Podman, Colima) and
run this again, or build the image on a Linux machine with packaging/vm/build-image.sh --arch $arch
--build-kernel (or download CI's vm-bundle-image-$arch artifact) and pass it as IMAGE=<dir>.
EOF
    exit 1
  fi
  case "$arch" in aarch64) plat=linux/arm64 ;; *) plat=linux/amd64 ;; esac
  echo "prepare-vm.sh: building the $arch guest image in a $plat $engine container → $img" >&2
  mkdir -p "$img"
  src="$root"; out="$img"
  # Docker Desktop on Windows wants native paths.
  if command -v cygpath >/dev/null 2>&1; then src="$(cygpath -w "$root")"; out="$(cygpath -w "$img")"; fi
  MSYS_NO_PATHCONV=1 "$engine" run --rm --platform "$plat" -v "$src:/src" -v "$out:/out" -w /src \
    -e SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct -- crates/seagent-vm crates/seagent-net packaging/vm/build-image.sh packaging/vm/kernel Cargo.lock 2>/dev/null || true)}" \
    ubuntu:24.04 bash -euc "
      export DEBIAN_FRONTEND=noninteractive
      apt-get update -q >/dev/null
      apt-get install -y -q --no-install-recommends build-essential flex bison bc libelf-dev cpio xz-utils musl-tools curl ca-certificates git python3 >/dev/null
      curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --target $arch-unknown-linux-musl >/dev/null
      . \$HOME/.cargo/env
      git config --global --add safe.directory /src
      export CC_${arch}_unknown_linux_musl=musl-gcc CARGO_TARGET_DIR=/src/target/vm-container-$arch
      packaging/vm/build-image.sh --out /out --arch $arch --build-kernel"
fi

out="$root/apps/desktop/src-tauri/vm"
mkdir -p "$root/target"
args=(--os "$os" --arch "$arch" --image "$img" --out "$out" --lock "$root/packaging/vm/image.lock")
[[ -n "${CACHE:-}" ]] && args+=(--cache "$CACHE")
"$root/packaging/vm/bundle.sh" "${args[@]}" | tee "$root/target/vm-bundle.out"
pin="$(sed -n 's/^SEAGENT_VM_IMAGE_MANIFEST_SHA256=//p' "$root/target/vm-bundle.out" | tail -1)"
echo "prepare-vm.sh: staged the $os VM in $out (manifest $pin); prepare-sidecar.sh $triple pins it."
