//! Compile Cap'n Proto schemas from a `build.rs` with **no system `capnp`** and
//! **no C/C++ toolchain**.
//!
//! The Cap'n Proto schema compiler is embedded as a single, architecture-
//! independent WebAssembly module (`capnp.wasm`) and executed *in-process* by a
//! pure-Rust WASM engine. Its `CodeGeneratorRequest` output is fed directly to
//! [`capnpc`]'s code generator — no subprocess, no native binary, no network.
//!
//! ```no_run
//! // build.rs
//! capnpc_embedded::CompileCommand::new()
//!     .file("schema/foo.capnp")
//!     .run()
//!     .expect("schema compilation failed");
//! ```
//!
//! The API mirrors [`capnpc::CompilerCommand`] so migrating is a one-line swap.

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

/// A builder for a schema-compilation command, mirroring
/// [`capnpc::CompilerCommand`].
#[derive(Default)]
pub struct CompileCommand {
    files: Vec<PathBuf>,
    src_prefixes: Vec<PathBuf>,
    import_paths: Vec<PathBuf>,
    output_path: Option<PathBuf>,
    default_parent_module: Vec<String>,
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

    /// Adds a `--src-prefix`: a prefix stripped from schema paths when computing
    /// output file names. See [`capnpc::CompilerCommand::src_prefix`].
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

    /// Sets the output directory. Defaults to the `OUT_DIR` environment variable.
    pub fn output_path<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.output_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the default parent module for generated code. See
    /// [`capnpc::CompilerCommand::default_parent_module`].
    pub fn default_parent_module(&mut self, default_parent_module: Vec<String>) -> &mut Self {
        self.default_parent_module = default_parent_module;
        self
    }

    /// Disables the embedded standard import schemas. Only set this if you are
    /// providing your own copies of `capnp/*.capnp` via [`Self::import_path`].
    pub fn no_standard_import(&mut self) -> &mut Self {
        self.no_standard_import = true;
        self
    }

    /// Runs the compilation: executes the embedded `capnp.wasm` in-process and
    /// generates Rust code into the output directory.
    pub fn run(&mut self) -> capnp::Result<()> {
        let output_path = match &self.output_path {
            Some(p) => p.clone(),
            None => PathBuf::from(std::env::var("OUT_DIR").map_err(|e| {
                capnp::Error::failed(format!(
                    "Could not access `OUT_DIR` environment variable: {e}. \
                     Set it up, or call `output_path` to choose an output directory."
                ))
            })?),
        };

        // Validate inputs up front for friendly errors (the wasm sees them via a
        // preopened directory).
        for file in &self.files {
            std::fs::metadata(file).map_err(|e| {
                capnp::Error::failed(format!(
                    "Unable to read capnp input file `{}`: {e}",
                    file.display()
                ))
            })?;
        }

        let request = runner::run_capnp(self).map_err(|e| {
            capnp::Error::failed(format!("Error running embedded capnp compiler: {e:#}"))
        })?;

        let mut codegen = capnpc::codegen::CodeGenerationCommand::new();
        codegen
            .output_directory(output_path)
            .default_parent_module(self.default_parent_module.clone());
        codegen.run(&request[..])
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
