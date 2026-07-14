//! Compile Cap'n Proto schemas from a `build.rs` with **no system `capnp`** and
//! **no C/C++ toolchain**.
//!
//! The Cap'n Proto schema compiler is embedded as a single, architecture-
//! independent WebAssembly module (`capnp.wasm`) and executed *in-process* by a
//! pure-Rust WASM engine. [`CompileCommand::compile`] returns the raw
//! `CodeGeneratorRequest` the compiler produces — no subprocess, no native
//! binary, no network.
//!
//! This crate deliberately does **not** depend on `capnpc`/`capnp`. You feed the
//! returned request to your own chosen `capnpc` version, so nothing here couples
//! you to a particular `capnp` runtime:
//!
//! ```no_run
//! // build.rs
//! let request = capnpc_embedded::CompileCommand::new()
//!     .src_prefix("schema")
//!     .file("schema/foo.capnp")
//!     .compile()
//!     .expect("capnp compile failed");
//!
//! capnpc::codegen::CodeGenerationCommand::new()
//!     .output_directory(std::env::var("OUT_DIR").unwrap())
//!     .run(&request[..])
//!     .expect("capnp codegen failed");
//! ```

use std::path::{Path, PathBuf};

mod runner;

/// The embedded standard Cap'n Proto import schemas (importable as
/// `/capnp/<name>.capnp`), including `rust.capnp` for the capnpc-rust
/// annotations (`$Rust.parentModule`, `$Rust.name`, ...).
const STD_SCHEMAS: &[(&str, &[u8])] = &[
    (
        "capnp/c++.capnp",
        include_bytes!("../assets/capnp-include/capnp/c++.capnp"),
    ),
    (
        "capnp/schema.capnp",
        include_bytes!("../assets/capnp-include/capnp/schema.capnp"),
    ),
    (
        "capnp/stream.capnp",
        include_bytes!("../assets/capnp-include/capnp/stream.capnp"),
    ),
    (
        "capnp/persistent.capnp",
        include_bytes!("../assets/capnp-include/capnp/persistent.capnp"),
    ),
    (
        "capnp/rpc.capnp",
        include_bytes!("../assets/capnp-include/capnp/rpc.capnp"),
    ),
    (
        "capnp/rpc-twoparty.capnp",
        include_bytes!("../assets/capnp-include/capnp/rpc-twoparty.capnp"),
    ),
    (
        "capnp/rust.capnp",
        include_bytes!("../assets/capnp-include/capnp/rust.capnp"),
    ),
];

/// An error produced while compiling schemas.
#[derive(Debug)]
pub struct Error {
    message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Result alias for schema compilation.
pub type Result<T> = std::result::Result<T, Error>;

/// A builder for compiling Cap'n Proto schemas into a `CodeGeneratorRequest`.
///
/// The builder options mirror the corresponding `capnp compile` flags. Options
/// that concern code *generation* (output directory, parent module, ...) live on
/// your `capnpc` code generator, not here — this type only runs the compiler.
#[derive(Default)]
pub struct CompileCommand {
    files: Vec<PathBuf>,
    src_prefixes: Vec<PathBuf>,
    import_paths: Vec<PathBuf>,
    no_standard_import: bool,
}

impl CompileCommand {
    /// Creates a new, empty command.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a schema file to compile. Paths are resolved relative to the current
    /// working directory (typically the crate root during a build script).
    pub fn file<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.files.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds a `--src-prefix`: a prefix stripped from schema paths when the
    /// compiler computes each file's display name (which in turn drives the
    /// generated file names).
    pub fn src_prefix<P: AsRef<Path>>(&mut self, prefix: P) -> &mut Self {
        self.src_prefixes.push(prefix.as_ref().to_path_buf());
        self
    }

    /// Adds an `--import-path` directory searched for absolute (`/foo.capnp`)
    /// imports.
    pub fn import_path<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.import_paths.push(dir.as_ref().to_path_buf());
        self
    }

    /// Disables the embedded standard import schemas. Only set this if you are
    /// providing your own copies of `capnp/*.capnp` via [`Self::import_path`].
    pub fn no_standard_import(&mut self) -> &mut Self {
        self.no_standard_import = true;
        self
    }

    /// Runs the embedded `capnp` compiler and returns the raw serialized
    /// `CodeGeneratorRequest` message.
    ///
    /// Feed the returned bytes to your code generator, e.g.
    /// `capnpc::codegen::CodeGenerationCommand::run(&request[..])`.
    pub fn compile(&self) -> Result<Vec<u8>> {
        // Validate inputs up front for friendly errors (the wasm sees them via a
        // preopened directory).
        for file in &self.files {
            std::fs::metadata(file).map_err(|e| {
                Error::new(format!(
                    "unable to read capnp input file `{}`: {e}",
                    file.display()
                ))
            })?;
        }

        runner::run_capnp(self)
            .map_err(|e| Error::new(format!("error running embedded capnp compiler: {e:#}")))
    }

    // --- accessors for the runner module ---
    pub(crate) fn files(&self) -> &[PathBuf] {
        &self.files
    }
    pub(crate) fn src_prefixes(&self) -> &[PathBuf] {
        &self.src_prefixes
    }
    pub(crate) fn import_paths(&self) -> &[PathBuf] {
        &self.import_paths
    }
    pub(crate) fn use_standard_import(&self) -> bool {
        !self.no_standard_import
    }
}
