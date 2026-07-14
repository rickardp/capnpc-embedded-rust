# capnpc-embedded

[![crates.io](https://img.shields.io/crates/v/capnpc-embedded.svg)](https://crates.io/crates/capnpc-embedded)
[![docs.rs](https://img.shields.io/docsrs/capnpc-embedded)](https://docs.rs/capnpc-embedded)
[![CI](https://github.com/rickardp/capnpc-embedded-rust/actions/workflows/test.yml/badge.svg)](https://github.com/rickardp/capnpc-embedded-rust/actions/workflows/test.yml)

Compile Cap'n Proto schemas from your `build.rs` with **no system `capnp`
installation** and **no C/C++ toolchain** — only a Rust toolchain.

The Cap'n Proto schema compiler is embedded as a single, architecture-independent
**WebAssembly** module and executed *in-process* by a pure-Rust WASM engine. No
subprocess, no native binary to install, and no network access at build time.

This crate is a **sidecar**: it only runs the compiler and hands you back the raw
`CodeGeneratorRequest`. You feed that to **your own** `capnpc`, so it does not
depend on — or pin the version of — `capnpc`/`capnp`. Pick whatever `capnp`
runtime your project uses. See capnproto-rust issue
[#182](https://github.com/capnproto/capnproto-rust/issues/182) for background.

## Usage

```toml
# Cargo.toml
[build-dependencies]
capnpc-embedded = "0.2"
capnpc = "0.20"          # your choice — this is what the generated code targets

[dependencies]
capnp = "0.20"
```

```rust
// build.rs
fn main() {
    // 1. Run the embedded compiler → raw CodeGeneratorRequest bytes.
    let request = capnpc_embedded::CompileCommand::new()
        .file("schema/foo.capnp")
        .src_prefix("schema")
        .compile()
        .expect("failed to compile capnp schema");

    // 2. Generate Rust with your own capnpc version.
    capnpc::codegen::CodeGenerationCommand::new()
        .output_directory(std::env::var("OUT_DIR").unwrap())
        .run(&request[..])
        .expect("failed to generate code");
}
```

```rust
// src/main.rs
mod foo_capnp {
    include!(concat!(env!("OUT_DIR"), "/foo_capnp.rs"));
}
```

`CompileCommand` mirrors the `capnp compile` flags (`file`, `src_prefix`,
`import_path`, `no_standard_import`). Code-*generation* options — output
directory, parent module, etc. — live on your `capnpc`
`CodeGenerationCommand`, not here.

The standard import schemas — including `rust.capnp` for the capnpc-rust
annotations (`$Rust.parentModule`, `$Rust.name`, ...) — are bundled, so
`import "/capnp/rust.capnp";` works out of the box.

## Why WebAssembly?

Shipping precompiled *native* binaries is fragile — musl vs glibc, NixOS's
`/nix/store` + `patchelf`, and per-arch matrices (exactly the concerns raised in
issue #182). A single `.wasm` sidesteps all of it: the same artifact runs
identically on Linux (glibc **and** musl), macOS, and Windows, on any CPU
architecture, with no toolchain and no network. It is verified in CI on
linux-glibc, Alpine/musl, macOS, and Windows.

## Versioning

Versions are `X.Y.Z+A.B.C`:

- **`X.Y.Z`** — this crate's own SemVer (its API and packaging).
- **`+A.B.C`** — the bundled Cap'n Proto compiler version (informational).

Cargo **ignores** build metadata for version resolution, so:

- Every release bumps the **`X.Y.Z`** part; the `+A.B.C` suffix is never the sole
  difference between two published versions.
- Pin the SemVer part (`capnpc-embedded = "0.2"`); you cannot pin "whatever
  bundles capnp 1.4". To pin an exact compiler, pin the exact crate version
  (`=0.1.0`) and read the metadata.

We follow SemVer for the crate itself. A new bundled capnp that is
backward-compatible is a patch/minor bump; one that changes generated output in a
breaking way (or an API break) is a breaking bump.

## How the wasm is built

`assets/capnp.wasm` is **generated, not committed to git**. `ci/build-wasm.sh`
downloads the pinned *pristine* capnproto release, applies a small
`__wasi__`-guarded patch (`patches/wasi-<version>.patch`, ~9 files), and builds
with wasi-sdk + CMake to `wasm32-wasip1`. The compiler is built without C++
exceptions (`-fno-exceptions`), so the result is a plain MVP wasm that runs on
any engine.

We do not vendor capnproto's source — only that patch. The release pipeline runs
`build-wasm.sh` before `cargo publish`, so the published crate on crates.io
already contains the prebuilt wasm; **consumers never run wasi-sdk or cmake.**

See [`docs/architecture.md`](docs/architecture.md) for the full design: the WASI
filesystem/path model, the wasmi engine and MVP-module constraints, and the
compiler patch.

## Developing

The wasm isn't in git, so a fresh checkout must produce it once:

```sh
ci/build-wasm.sh   # needs cmake; downloads wasi-sdk
cargo test
```

## Platforms

Linux (glibc **and** musl), macOS, and Windows, on any CPU architecture — the
same WebAssembly artifact everywhere. Every platform is exercised end-to-end in
CI (including Alpine/musl).

## Limitations

- Schema files and their imports must live on the local filesystem (they do, for
  a normal build). The compiler runs with a read-only view of the filesystem.
- On Windows, all input schemas/imports must be on a single drive (the current
  directory's drive) — normal for a project build.
- Only the schema-compile path is supported (external `capnp` code-generator
  *plugins* are not — but `capnpc` generates Rust in-process, so this is not a
  limitation for Rust codegen).

## License

MIT — see [`LICENSE`](LICENSE).

This crate bundles and redistributes MIT-licensed components from Cap'n Proto
(the `capnp.wasm` compiler and the standard `capnp/*.capnp` schemas) and
capnproto-rust (`rust.capnp`). Their copyright notices are reproduced in
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
