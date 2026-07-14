# Architecture

How `capnpc-embedded` compiles Cap'n Proto schemas with only a Rust toolchain,
and how the embedded WebAssembly compiler is produced.

## Overview

`capnpc-embedded` supplies the Cap'n Proto schema *compiler* without requiring a
system `capnp` install or a C/C++ toolchain. The `capnp` tool is shipped as a
single, architecture-independent WebAssembly module and executed **in-process**
by the pure-Rust [`wasmi`](https://docs.rs/wasmi) interpreter.

It is a **sidecar**: `CompileCommand::compile()` returns the raw
`CodeGeneratorRequest` bytes and stops there. The crate does **not** depend on
`capnpc`/`capnp`; the caller feeds the request to their own chosen `capnpc`
version. This fully decouples us from the `capnp` runtime version.

```
CompileCommand::compile()  ── this crate ──
  ├─ stage bundled standard schemas (capnp/*.capnp, rust.capnp) to a temp dir
  ├─ run capnp.wasm via wasmi + WASI:  `capnp compile -o- <files>`
  │     └─ capnp writes a CodeGeneratorRequest to stdout (captured in memory)
  └─ return the CodeGeneratorRequest bytes

── caller's build.rs (their own capnpc) ──
  capnpc::codegen::CodeGenerationCommand::run(&request)  → *.rs in OUT_DIR
```

The interchange point is the `CodeGeneratorRequest` (the `schema.capnp` wire
format), which any `capnpc` version reads via
`capnpc::codegen::CodeGenerationCommand::run<T: Read>`. Because of it we never
need `capnp` to exist as a spawnable native executable — no subprocess, no native
binary, no PATH lookup — and we never link `capnpc`.

## Why WebAssembly

Shipping precompiled *native* binaries is fragile — musl vs glibc, NixOS's
`/nix/store` + `patchelf`, and a per-architecture build matrix. A single `.wasm`
sidesteps all of it: the same artifact runs identically on Linux (glibc **and**
musl), macOS, and Windows, on any CPU architecture, with no toolchain and no
network at build time. This is verified end-to-end in CI across those platforms
(including Alpine/musl).

## Engine: wasmi, and a plain MVP module

The crate uses `wasmi`, a pure-Rust interpreter: a lean build-dependency tree
(no cranelift) and fast startup. In practice schema compilation runs in roughly
0.3 s versus ~8 s with a JIT engine that recompiles the ~3 MB module on every
invocation — a meaningful per-build saving.

`wasmi` supports only MVP WebAssembly, so `capnp.wasm` is built as a plain MVP
module:

- **No C++ exceptions** (`-fno-exceptions -DKJ_NO_EXCEPTIONS=1`). Most engines do
  not implement the WASM exception-handling proposal; avoiding it keeps the module
  portable. The schema compiler reports user errors through KJ's `ProcessContext`,
  not C++ exceptions, so normal compilation is unaffected.
- **No atomics** (`-Xclang -target-feature -Xclang -atomics`). Cap'n Proto's
  `std::atomic`-based `ReadLimiter` otherwise emits atomic ops that `wasmi`
  rejects. Disabling the atomics target feature lowers them to plain loads/stores,
  which is correct because we run single-threaded on unshared memory.

## The WASI filesystem model and cross-platform paths

WASI exposes a single filesystem root to the guest, and the capnp compiler
resolves every path through that one root (a *second* preopened directory is
invisible to it). So the runner preopens exactly one host root, read-only, and
passes every path as an absolute path translated into that root:

- **Unix:** the root is `/`. Host paths map directly.
- **Windows:** the root is the drive of the current directory (e.g. `C:\`). Paths
  are translated to POSIX form via `Path::components`, which drops the drive and
  the `\\?\` verbatim prefix that `canonicalize()` adds (`\\?\C:\a\b` → `/a/b`).

The bundled standard-import schemas (`capnp/*.capnp` plus `rust.capnp` for the
capnpc-rust annotations) are staged into a temp directory **on the same
drive/root**, so they are reachable through that single root. `--no-standard-import`
is always passed, since the wasm's baked-in include directory does not exist at
runtime.

**Limitation:** all input schemas and their imports must live on a single
drive/root (normal for a project build). Only the schema-compile path is
supported — external code-generator *plugins* are not, but `capnpc` generates
Rust in-process, so that is not a limitation for Rust codegen.

## Building the embedded wasm

`assets/capnp.wasm` is generated, not committed to git. It is produced by
[`ci/build-wasm.sh`](../ci/build-wasm.sh), which:

1. Downloads the pinned **pristine** Cap'n Proto release
   (`CAPNP_VERSION` in the script).
2. Applies [`patches/wasi-<version>.patch`](../patches) — a small,
   `__wasi__`-guarded diff (~9 files) that ports the compiler to WASI.
3. Builds `capnp_tool` with wasi-sdk + CMake for `wasm32-wasip1` using the flags
   above, plus a 4 MB stack (`-z stack-size`; the 64 KB default overflows during
   `newDiskFilesystem()` init) and a 32 MB initial memory.

We do **not** vendor Cap'n Proto's source — only the patch. The release pipeline
runs `build-wasm.sh` before `cargo publish`, so the published crate on crates.io
already contains the prebuilt wasm; consumers never run wasi-sdk or cmake.

### What the WASI patch changes

All changes are `#[cfg]`/`#if defined(__wasi__)`-guarded and do not affect other
targets:

| Area | Change |
|---|---|
| Crash/signal handlers (`kj/exception.c++`) | stubbed; no POSIX signals on WASI |
| `kj/miniposix.h`, `kj/thread.c++` | `pipe`/`iovMax` stubs; guard `pthread_kill` |
| `kj/test-helpers.c++` | treat WASI like Windows for fork-based death tests |
| `kj/filesystem-disk-unix.c++` | `dup`/`mknodat`/`msync`/`getpid` stubs; `computeCurrentPath` returns root (no `getcwd`); ignore emulated-mman `munmap` EINVAL; skip the qemu root-fd heuristic |
| `capnp/message.h` | `arenaSpacePadding` for the wasm32 layout |
| `capnp/compiler/parser.c++` | `getentropy()` instead of `/dev/urandom` |
| `capnp/compiler/capnp.c++` | only the `-o-` (stdout) output path; no plugin fork/exec |
| `capnp/CMakeLists.txt` | drop the over-declared `kj-async` dependency of `capnp-json` |

## Versioning

Versions are `X.Y.Z+A.B.C`: `X.Y.Z` is this crate's own SemVer, `+A.B.C` is the
bundled Cap'n Proto version (informational). Cargo ignores build metadata for
resolution, so **every release bumps the SemVer part**; the `+A.B.C` suffix is
never the only difference between two published versions. Consumers pin the SemVer
part (`capnpc-embedded = "0.1"`).

## Keeping the bundled compiler current

Dependabot handles the crate's own Cargo/Actions dependencies but cannot track
upstream Cap'n Proto *release tags*. A weekly workflow
([`capnp-release-watch.yml`](../.github/workflows/capnp-release-watch.yml)) opens
an issue when a newer Cap'n Proto release is available; updating then means
bumping `CAPNP_VERSION`, porting the patch, regenerating the wasm, and bumping the
crate's SemVer.
