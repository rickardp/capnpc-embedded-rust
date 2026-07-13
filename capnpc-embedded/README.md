# capnpc-embedded

Compile Cap'n Proto schemas from your `build.rs` with **no system `capnp`
installation** and **no C/C++ toolchain** — only a Rust toolchain.

The Cap'n Proto schema compiler is embedded as a single, architecture-independent
**WebAssembly** module and executed *in-process* by a pure-Rust WASM engine. Its
output is fed straight into [`capnpc`]'s code generator. No subprocess, no native
binary to install, and no network access at build time.

This is a companion to `capnpc` for people who want self-contained, reproducible
builds and don't want a system-package dependency. It does not modify or replace
`capnpc`; it just supplies the compiler. See capnproto-rust issue
[#182](https://github.com/capnproto/capnproto-rust/issues/182) for background.

## Usage

```toml
# Cargo.toml
[build-dependencies]
capnpc-embedded = "0.1"

[dependencies]
capnp = "0.20"
```

```rust
// build.rs
fn main() {
    capnpc_embedded::CompileCommand::new()
        .file("schema/foo.capnp")
        .src_prefix("schema")
        .run()
        .expect("failed to compile capnp schema");
}
```

```rust
// src/main.rs
mod foo_capnp {
    include!(concat!(env!("OUT_DIR"), "/foo_capnp.rs"));
}
```

The API mirrors [`capnpc::CompilerCommand`] (`file`, `src_prefix`, `import_path`,
`output_path`, `default_parent_module`), so migrating an existing build script is
a one-line swap.

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
- Pin the SemVer part (`capnpc-embedded = "0.1"`); you cannot pin "whatever
  bundles capnp 1.4". To pin an exact compiler, pin the exact crate version
  (`=0.1.0`) and read the metadata.

We follow SemVer for the crate itself. A new bundled capnp that is
backward-compatible is a patch/minor bump; one that changes generated output in a
breaking way (or an API break) is a breaking bump.

## How the wasm is built

`assets/capnp.wasm` is produced by `ci/build-wasm.sh`, which downloads the pinned
capnproto release, applies a small `__wasi__`-guarded patch
(`vendor/wasi-<version>.patch`, ~9 files), and builds with wasi-sdk + CMake to
`wasm32-wasip1`. The compiler is built without C++ exceptions
(`-fno-exceptions`), so the result is a plain MVP wasm that runs on any engine.

## Limitations

- Schema files and their imports must live on the local filesystem (they do, for
  a normal build). The compiler runs with a read-only view of the filesystem.
- Only the schema-compile path is supported (external `capnp` code-generator
  *plugins* are not — but `capnpc` generates Rust in-process, so this is not a
  limitation for Rust codegen).

## License

MIT. Bundles Cap'n Proto (MIT) and its standard schemas.

[`capnpc`]: https://docs.rs/capnpc
[`capnpc::CompilerCommand`]: https://docs.rs/capnpc/latest/capnpc/struct.CompilerCommand.html
