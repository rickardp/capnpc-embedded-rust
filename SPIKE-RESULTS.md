# WASI spike results — capnp compiler as portable WASM

**Outcome: SUCCESS, end-to-end, on all platforms tested.**

Goal was to prove we can ship the Cap'n Proto schema compiler as a single
architecture-independent WebAssembly artifact, run it in-process from a Rust
build script with only a Rust toolchain, and feed its output to `capnpc` — with
no C/C++ toolchain, no network at build time, and no per-platform native binary.

## What was proven

1. **capnproto 1.4.0 `capnp` tool compiles to `wasm32-wasip1`** with wasi-sdk-33
   and a small, well-contained patch set (see below). Output: a **3.2 MB MVP
   WebAssembly binary** — no exceptions, no threads-proposal dependency beyond
   atomics.
2. **It runs under the wasmtime CLI** and compiles a real schema:
   `capnp compile -o- schema/foo.capnp` emits a valid `CodeGeneratorRequest`
   (1560 bytes) that **native capnp `decode` parses correctly** (structs Point,
   Line present).
3. **It runs in-process from Rust** via the `wasmtime` crate + `wasmtime-wasi`
   (preview1): preopen the schema dir, capture stdout, hand the bytes to
   `capnpc::codegen::CodeGenerationCommand::run(&request[..])`. Produced 520
   lines of Rust with `point`/`line` modules and Reader/Builder impls.
4. **The generated Rust compiles and type-checks** against the `capnp` crate
   (built a lib that round-trips a `Line` message: set/get fields — OK).
5. **Portable across the axes that break native binaries.** The *same* `.wasm`
   produced the *identical* 1560-byte request on:
   - macOS arm64 (wasmtime CLI + Rust host)
   - **Alpine aarch64 (musl)** — via OrbStack/Docker
   - **Debian 12 aarch64 (glibc 2.36)**
   - **Alpine x86_64 (musl, emulated)** — different arch entirely
   No musl/glibc/patchelf/per-arch issues, because the artifact is wasm.

## Key technical decision: no C++ exceptions

wasmtime 28 (and most engines today) do **not** support the WASM
exception-handling proposal. Rather than require a bleeding-edge engine (which
would undercut portability), we build capnp with `-fno-exceptions
-DKJ_NO_EXCEPTIONS=1`. KJ supports this config; the schema compiler reports user
errors through its `ProcessContext`, not C++ exceptions, so normal compilation is
unaffected. Result: a pure MVP wasm that runs on *any* engine (wasmtime, and
`wasmi` the pure-Rust interpreter — relevant for lean build-dependency trees).

## Build recipe (for the packaging crate's release CI)

Toolchain: wasi-sdk (clang + sysroot), cmake.

```
cmake <capnp>/c++ \
  -DCMAKE_TOOLCHAIN_FILE=$WASI/share/cmake/wasi-sdk.cmake -DWASI_SDK_PREFIX=$WASI \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF \
  -DWITH_OPENSSL=OFF -DWITH_ZLIB=OFF -DWITH_FIBERS=OFF \
  -DCMAKE_CXX_FLAGS="-D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_MMAN \
     -D_WASI_EMULATED_PROCESS_CLOCKS -DKJ_NO_EXCEPTIONS=1 -fno-exceptions" \
  -DCMAKE_EXE_LINKER_FLAGS="-lwasi-emulated-signal -lwasi-emulated-mman \
     -lwasi-emulated-process-clocks -Wl,-z,stack-size=4194304 \
     -Wl,--initial-memory=33554432"
make capnp_tool     # -> src/capnp/capnp  (a .wasm named `capnp`)
```

Two runtime settings that mattered:
- **Stack size 4 MB** (`-z stack-size`): the wasi-sdk default 64 KB overflowed
  during `newDiskFilesystem()` init and faulted. 4 MB is comfortable.
- **Initial memory 32 MB**: avoids early `memory.grow` churn.
- wasmtime host must enable **threads/atomics** (`Config::wasm_threads(true)`);
  kj's mutex emits atomic ops even single-threaded.

## Patch set against capnproto 1.4.0 (all `#if defined(__wasi__)` guarded)

Small, upstreamable, and non-invasive — ~9 files:

| File | Change |
|---|---|
| `kj/exception.c++` | stub crash/signal handlers; skip `__builtin_return_address` |
| `kj/miniposix.h` | `pipe()` stub + `iovMax()` for wasi |
| `kj/thread.c++` | guard `<signal.h>` + `pthread_kill` in `sendSignal` |
| `kj/test-helpers.c++` | treat wasi like Windows for fork-based death tests |
| `kj/filesystem-disk-unix.c++` | `dup`/`mknodat`/`msync`/`getpid` stubs; `computeCurrentPath` returns root (no `getcwd`) |
| `capnp/message.h` | `arenaSpacePadding = 19` on wasi (layout) |
| `capnp/compiler/parser.c++` | `getentropy()` instead of `/dev/urandom` |
| `capnp/compiler/capnp.c++` | guard `<sys/wait.h>`; only `-o-` output (no plugin fork/exec) |
| `capnp/CMakeLists.txt` | drop over-declared `kj-async` dep from `capnp-json` |

## WASI usage notes (for the crate's runner)

- capnp has **no cwd**; run with the schema root preopened as guest `/` and the
  current path treated as root. Pass schema paths relative to that preopen.
- Only `capnp compile -o-` (CodeGeneratorRequest to stdout) is supported — the
  external-plugin fork/exec path is compiled out. This is exactly what capnpc's
  in-process codegen needs, so it's not a limitation for us.
- A benign warning prints on startup ("root dir file descriptor is broken,
  probably because of qemu; compensating") from a `stat("/dev/..")` heuristic;
  harmless, can be guarded out later.

## Artifacts from the spike (in scratchpad)

- `build-wasi/src/capnp/capnp` — the 3.2 MB wasm
- `inproc/` — Rust host: wasmtime + capture stdout + capnpc codegen
- `compilecheck/` — proves generated Rust compiles against the capnp crate
