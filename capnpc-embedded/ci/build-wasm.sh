#!/usr/bin/env bash
#
# Reproducibly build assets/capnp.wasm: the Cap'n Proto schema compiler compiled
# to wasm32-wasip1. Run from anywhere; writes to capnpc-embedded/assets/capnp.wasm.
#
# Requirements: cmake, curl, tar, and a wasi-sdk (auto-downloaded if absent).
#
# The version numbers below are the single source of truth for what gets bundled;
# bump CAPNP_VERSION when tracking a new upstream release (and the crate version's
# +metadata to match).
set -euo pipefail

CAPNP_VERSION="${CAPNP_VERSION:-1.4.0}"
WASI_SDK_VERSION="${WASI_SDK_VERSION:-33}"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # capnpc-embedded/
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# --- wasi-sdk -----------------------------------------------------------------
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  wasi_host="arm64-macos" ;;
  Darwin-x86_64) wasi_host="x86_64-macos" ;;
  Linux-x86_64)  wasi_host="x86_64-linux" ;;
  Linux-aarch64) wasi_host="arm64-linux" ;;
  *) echo "unsupported host for wasi-sdk auto-download: $(uname -sm)" >&2; exit 1 ;;
esac
WASI_SDK="${WASI_SDK:-}"
if [[ -z "$WASI_SDK" ]]; then
  echo ">> fetching wasi-sdk-${WASI_SDK_VERSION} ($wasi_host)"
  url="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-${WASI_SDK_VERSION}/wasi-sdk-${WASI_SDK_VERSION}.0-${wasi_host}.tar.gz"
  curl -fsSL "$url" | tar xz -C "$work"
  WASI_SDK="$work/wasi-sdk-${WASI_SDK_VERSION}.0-${wasi_host}"
fi

# --- capnproto source + our WASI patch ----------------------------------------
echo ">> fetching capnproto ${CAPNP_VERSION}"
curl -fsSL "https://github.com/capnproto/capnproto/archive/refs/tags/v${CAPNP_VERSION}.tar.gz" \
  | tar xz -C "$work"
src="$work/capnproto-${CAPNP_VERSION}/c++"

echo ">> applying WASI patch"
patch -p1 -d "$src" < "$here/patches/wasi-${CAPNP_VERSION}.patch"

# --- build --------------------------------------------------------------------
echo ">> configuring (cmake + wasi-sdk)"
build="$work/build"
cmake -S "$src" -B "$build" -G "Unix Makefiles" \
  -DCMAKE_TOOLCHAIN_FILE="$WASI_SDK/share/cmake/wasi-sdk.cmake" \
  -DWASI_SDK_PREFIX="$WASI_SDK" \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF \
  -DWITH_OPENSSL=OFF -DWITH_ZLIB=OFF -DWITH_FIBERS=OFF \
  -DCMAKE_CXX_FLAGS="-D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_MMAN -D_WASI_EMULATED_PROCESS_CLOCKS -DKJ_NO_EXCEPTIONS=1 -fno-exceptions" \
  -DCMAKE_EXE_LINKER_FLAGS="-lwasi-emulated-signal -lwasi-emulated-mman -lwasi-emulated-process-clocks -Wl,-z,stack-size=4194304 -Wl,--initial-memory=33554432"

echo ">> building capnp_tool"
cmake --build "$build" --target capnp_tool -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"

mkdir -p "$here/assets"
cp "$build/src/capnp/capnp" "$here/assets/capnp.wasm"
echo ">> wrote $here/assets/capnp.wasm ($(wc -c < "$here/assets/capnp.wasm") bytes)"

# --- refresh bundled standard schemas -----------------------------------------
for f in c++ schema stream persistent rpc rpc-twoparty; do
  cp "$src/src/capnp/$f.capnp" "$here/assets/capnp-include/capnp/"
done
echo ">> refreshed standard import schemas (rust.capnp is tracked separately)"
