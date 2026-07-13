# Plan: a `capnpc-embedded` packaging crate for capnproto-rust

> Updated after the WASI spike (see `SPIKE-RESULTS.md`). The spike validated the
> WASM approach end-to-end on macOS, Alpine/musl, Debian/glibc, and x86_64. This
> plan now targets that architecture.

## Goal

Let a Rust project compile its `.capnp` schemas in `build.rs` with **only a Rust
toolchain** — no system `capnp`, no C/C++ toolchain, and no network at build
time — while *not* forking capnproto-rust and *not* fighting the maintainer's
stance in issue #182 (opposed to anything that pushes users off system packages
*by default*). The crate is **opt-in**; `capnpc` is unchanged.

## Architecture (validated)

Ship the Cap'n Proto schema compiler as a single **architecture-independent
`capnp.wasm`** (`wasm32-wasip1`, MVP, no exceptions/threads), `include_bytes!`'d
into the crate. At consumer build time, run it **in-process** with a pure-Rust
WASM engine, capture the `CodeGeneratorRequest` from its stdout, and feed those
bytes to `capnpc::codegen::CodeGenerationCommand::run(&request[..])`.

Why this shape (all confirmed by the spike):
- **Rust-only, no network, no C++ toolchain** — the wasm is prebuilt once in our
  CI; consumers just run it.
- **Portable across the axes that break native binaries** — same `.wasm` gave
  identical output on musl, glibc, and a different CPU arch. No
  musl/glibc/NixOS/`patchelf` failure modes (the objections in #182).
- **No subprocess / no native launcher / no shim** — because `capnpc::codegen`
  accepts a `Read`, we run the wasm in-process and never need `capnp` to exist as
  a spawnable native executable. This is the detail that makes the WASM route
  clean rather than clunky.

### Integration seam in capnpc

`capnpc::codegen::CodeGenerationCommand::run<T: std::io::Read>(inp)` is public and
stable (`capnpc/src/codegen.rs`). We do NOT use `CompilerCommand::capnp_executable`
(that spawns a subprocess) — we go straight to `codegen`.

## Engine choice — decided: wasmi

The crate uses **`wasmi`** (pure-Rust interpreter). To make this work the wasm is
built as a plain MVP module: no C++ exceptions (`-fno-exceptions`) **and no
atomics** (`-Xclang -target-feature -Xclang -atomics`, safe because we run
single-threaded — capnp's `std::atomic` ReadLimiter otherwise emits atomic ops
that wasmi rejects). One extra WASI fix was needed: the emulated-mman `munmap`
returns EINVAL, so the mmap disposer ignores it (short-lived process, harmless
leak).

Payoff vs the wasmtime spike: no cranelift in the dependency tree, and schema
compilation runs in **~0.3 s instead of ~8 s** per build (wasmtime JIT-compiled
the 3 MB module every invocation; wasmi interprets it directly). Adequate speed
for build-time codegen.

## Crate layout

Standalone repo, versioned `X.Y.Z+<capnp-version>` (e.g. `0.1.0+1.4.0`):

```
capnpc-embedded/
  Cargo.toml
  build.rs                 # errors clearly if capnp.wasm hasn't been produced
  src/lib.rs               # CompileCommand API + in-process wasm runner
  assets/capnp.wasm        # generated (gitignored); include_bytes!'d (~3.2 MB)
  assets/capnp-include/    # bundled standard schemas (schema/c++/rust.capnp, ...)
  patches/wasi-<ver>.patch # our ~9-file __wasi__ diff (NOT vendored capnp source)
  ci/build-wasm.sh         # fetches pristine capnp, applies patch, builds the wasm
  README.md
```

We do **not** vendor the capnproto crate or its C++ source. `build-wasm.sh`
downloads the pristine upstream release at build time and applies only
`patches/wasi-<ver>.patch` on top — a sibling/companion, not a fork.

The wasm is **not committed to git**. It is produced by our release pipeline
before `cargo publish` and ships inside the published `.crate`, so consumers get
it prebuilt and never run wasi-sdk/cmake themselves. (Ideal end state: upstream
the patch so we build straight from an unmodified release.)

## Downstream usage

```toml
[build-dependencies]
capnpc    = "0.20"
capnpc-embedded = "0.1"
```

```rust
// consumer build.rs
fn main() {
    capnpc_embedded::CompileCommand::new()
        .file("schema/foo.capnp")
        .run()               // runs capnp.wasm in-process, generates via capnpc
        .expect("schema compilation failed");
}
```

The API mirrors `capnpc::CompilerCommand` (file/src_prefix/import_path/output) so
migration is a one-line swap.

## Keeping the vendored capnp version current (Dependabot?)

Dependabot is a **partial** fit — right for our own deps, wrong for tracking
capnproto releases:

- ✅ `package-ecosystem: cargo` — keep `wasmi`/`wasmtime`/`capnpc` current.
- ✅ `package-ecosystem: github-actions` — keep the CI wasm-build workflow current.
- ⚠️ `gitsubmodule` — only bumps to the latest *commit* on a branch, not release
  tags; noisy and wrong for a tag-pinned vendored compiler.
- ❌ No native support for "new capnproto **release tag**", which is the event
  that actually triggers a wasm rebuild.

So: add `dependabot.yml` (cargo + github-actions), and add a **scheduled GitHub
Action** (weekly cron) that queries the `capnproto/capnproto` Releases API,
compares against the `+<version>` build-metadata in our crate version, and opens
an issue/PR when upstream ships a newer release. That PR runs `ci/build-wasm.sh`
to regenerate `assets/capnp.wasm`. (Same manual-on-release model `protobuf-src`
uses, with automation to notice the release.)

## Open items / follow-ups

1. ~~Swap the spike's wasmtime runner for `wasmi`.~~ **Done** — see Engine choice.
2. ~~Guard out the `stat("/dev/..")` qemu-compensation warning for wasi.~~ **Done**
   (it was corrupting the root fd; disabled for `__wasi__`).
3. ~~Decide wasm delivery.~~ **Done** — generated by `ci/build-wasm.sh`, gitignored,
   bundled into the published crate by the release job; never built by consumers.
4. Try to **upstream the ~9-file wasi patch set** to capnproto — it's small and
   `__wasi__`-guarded, so it doesn't affect existing targets and would remove our
   need to maintain a vendored patch. Independent of the maintainer's install
   stance (this is the C++ repo, not capnproto-rust).
5. Reproducibility: pin wasi-sdk version; consider building the wasm in CI and
   checking the digest.

## Engagement with upstream capnproto-rust

No code PR to capnproto-rust needed. Worth a follow-up on #182 announcing
`capnpc-embedded` once published — opt-in, matching the maintainer's position, and it
finally gives the "capnp not found" error a crate to point at (zenhack's original
suggestion).
