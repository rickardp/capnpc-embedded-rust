//! Runs the embedded `capnp.wasm` in-process via wasmtime and returns the raw
//! `CodeGeneratorRequest` it writes to stdout.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::pipe::MemoryOutputPipe;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use crate::{CompileCommand, STD_SCHEMAS};

/// Compile the command's schema files and return the raw `CodeGeneratorRequest`.
///
/// WASI exposes a single filesystem root to the guest, and the capnp compiler
/// resolves every path through that one root. So we preopen the host root
/// read-only and pass every path as an absolute, canonicalized path. This lets
/// capnp reach both the user's schema files and our staged standard-import
/// directory (which live in unrelated parts of the filesystem) through one root.
pub(crate) fn run_capnp(cmd: &CompileCommand) -> Result<Vec<u8>> {
    let cwd = std::env::current_dir().context("could not determine current directory")?;

    // Extract the bundled standard schemas to a temp dir (the wasm needs them on
    // a real filesystem to read).
    let std_dir = if cmd.use_standard_import() {
        Some(extract_std_schemas().context("failed to stage standard import schemas")?)
    } else {
        None
    };

    // Assemble the capnp command line, mirroring capnpc::CompilerCommand. All
    // paths are absolute + canonicalized so they resolve identically to how the
    // src-prefix stripping computes output names.
    let mut args: Vec<String> = vec![
        "capnp".into(),
        "compile".into(),
        "-o".into(),
        "-".into(),
        // We provide standard imports ourselves (or the user opted out), so the
        // wasm's baked-in include dir (absent at runtime) is never consulted.
        "--no-standard-import".into(),
    ];
    if let Some(dir) = &std_dir {
        args.push(format!("--import-path={}", to_guest(dir.path())));
    }
    for ip in cmd.import_paths() {
        args.push(format!("--import-path={}", to_guest(&abs(&cwd, ip)?)));
    }
    for sp in cmd.src_prefixes() {
        args.push(format!("--src-prefix={}", to_guest(&abs(&cwd, sp)?)));
    }
    for f in cmd.files() {
        args.push(to_guest(&abs(&cwd, f)?));
    }

    // Capture stdout (the CodeGeneratorRequest).
    let stdout = MemoryOutputPipe::new(64 * 1024 * 1024);

    let mut builder = WasiCtxBuilder::new();
    builder
        .stdout(stdout.clone())
        .inherit_stderr()
        .env("PWD", "/")
        .args(&args)
        .preopened_dir("/", "/", DirPerms::READ, FilePerms::READ)
        .context("failed to preopen filesystem root for the capnp compiler")?;
    let wasi: WasiP1Ctx = builder.build_p1();

    let mut config = Config::new();
    config.wasm_backtrace(true);
    config.wasm_threads(true); // kj emits atomic ops even single-threaded
    let engine = Engine::new(&config).context("failed to create wasm engine")?;
    let module =
        Module::from_binary(&engine, CAPNP_WASM).context("failed to load embedded capnp.wasm")?;

    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    preview1::add_to_linker_sync(&mut linker, |t| t)?;

    let mut store = Store::new(&engine, wasi);
    let instance = linker.instantiate(&mut store, &module)?;
    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

    // WASI `_start` ends by calling `proc_exit`, surfaced as an `I32Exit` error.
    if let Err(e) = start.call(&mut store, ()) {
        match e.downcast_ref::<wasmtime_wasi::I32Exit>() {
            Some(exit) if exit.0 == 0 => {}
            Some(exit) => {
                return Err(anyhow!(
                    "capnp compiler exited with code {} (see stderr above)",
                    exit.0
                ))
            }
            None => return Err(e.context("capnp compiler trapped")),
        }
    }

    drop(store);
    let bytes = stdout.contents().to_vec();
    if bytes.is_empty() {
        return Err(anyhow!(
            "capnp compiler produced no output; see stderr above for schema errors"
        ));
    }
    Ok(bytes)
}

/// The embedded WebAssembly build of the `capnp` schema compiler.
static CAPNP_WASM: &[u8] = include_bytes!("../assets/capnp.wasm");

/// Absolutize and canonicalize a (possibly relative) host path against `cwd`.
fn abs(cwd: &Path, p: &Path) -> Result<std::path::PathBuf> {
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };
    joined
        .canonicalize()
        .with_context(|| format!("could not resolve path `{}`", p.display()))
}

/// Convert a host path to the guest (unix, forward-slash) path used inside the
/// wasm. The host root is mounted at `/`, so canonical absolute paths map
/// directly; we only normalize Windows separators.
fn to_guest(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn extract_std_schemas() -> Result<tempfile::TempDir> {
    let dir = tempfile::Builder::new()
        .prefix("capnpc-embedded-std-")
        .tempdir()?;
    for (rel, bytes) in STD_SCHEMAS {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes)?;
    }
    Ok(dir)
}
