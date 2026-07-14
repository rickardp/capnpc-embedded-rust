//! Runs the embedded `capnp.wasm` in-process via the wasmi interpreter and
//! returns the raw `CodeGeneratorRequest` it writes to stdout.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use wasmi::{Config, Engine, Linker, Module, Store};
use wasmi_wasi::wasi_common::pipe::WritePipe;
use wasmi_wasi::{WasiCtx, WasiCtxBuilder};

use crate::{CompileCommand, STD_SCHEMAS};

/// Compile the command's schema files and return the raw `CodeGeneratorRequest`.
///
/// WASI exposes a single filesystem root to the guest, and the capnp compiler
/// resolves every path through that one root. So we preopen one host root and
/// pass every path as an absolute, canonicalized path translated into that root.
/// This lets capnp reach both the user's schema files and our staged
/// standard-import directory (in unrelated parts of the filesystem) through one
/// root.
///
/// - On Unix the root is `/`.
/// - On Windows the root is the drive of the current directory (e.g. `C:\`),
///   and paths are translated to POSIX form (`C:\a\b` -> `/a/b`). The staged
///   standard-import directory is created on that same drive so it is reachable.
///
/// Limitation: inputs must live on a single drive/root (they do for a normal
/// project build).
pub(crate) fn run_capnp(cmd: &CompileCommand) -> Result<Vec<u8>> {
    let cwd = std::env::current_dir().context("could not determine current directory")?;
    let root_host = preopen_root(&cwd)?;

    // Extract the bundled standard schemas to a temp dir (the wasm needs them on
    // a real filesystem to read). It must sit under `root_host` to be reachable.
    let std_dir = if cmd.use_standard_import() {
        Some(extract_std_schemas(&cwd).context("failed to stage standard import schemas")?)
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
        args.push(format!(
            "--import-path={}",
            to_guest(&abs(&cwd, dir.path())?)
        ));
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
    let stdout = WritePipe::new_in_memory();

    // Preopen the host root read-only (see the doc comment above for why).
    let root = Dir::open_ambient_dir(&root_host, ambient_authority()).with_context(|| {
        format!(
            "failed to open filesystem root `{}` for the capnp compiler",
            root_host.display()
        )
    })?;
    let mut builder = WasiCtxBuilder::new();
    builder
        .stdout(Box::new(stdout.clone()))
        .inherit_stderr()
        .env("PWD", "/")
        .context("failed to set environment for the capnp compiler")?
        .args(&args)
        .context("failed to set arguments for the capnp compiler")?
        .preopened_dir(root, "/")
        .context("failed to preopen filesystem root for the capnp compiler")?;
    let wasi: WasiCtx = builder.build();

    let engine = Engine::new(&Config::default());
    let module = Module::new(&engine, CAPNP_WASM).context("failed to load embedded capnp.wasm")?;

    let mut linker: Linker<WasiCtx> = Linker::new(&engine);
    wasmi_wasi::add_to_linker(&mut linker, |ctx| ctx)
        .map_err(|e| anyhow!("failed to link WASI: {e}"))?;

    let mut store = Store::new(&engine, wasi);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|e| anyhow!("failed to instantiate capnp.wasm: {e}"))?;
    let start = instance.get_typed_func::<(), ()>(&store, "_start")?;

    // WASI `_start` ends by calling `proc_exit`, surfaced as an error carrying
    // the exit code.
    if let Err(e) = start.call(&mut store, ()) {
        match e.i32_exit_status() {
            Some(0) => {}
            Some(code) => {
                return Err(anyhow!(
                    "capnp compiler exited with code {code} (see stderr above)"
                ))
            }
            None => return Err(anyhow!("capnp compiler trapped: {e}")),
        }
    }

    drop(store);
    let bytes = stdout
        .try_into_inner()
        .map_err(|_| anyhow!("stdout pipe still had other references"))?
        .into_inner();
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

/// The host directory to preopen as the guest's `/`.
///
/// Unix: `/`. Windows: the drive root of `cwd` (e.g. `C:\`), so that both the
/// project files and our staged standard schemas (created on the same drive) are
/// reachable through one WASI root.
fn preopen_root(cwd: &Path) -> Result<PathBuf> {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let canon = cwd
            .canonicalize()
            .with_context(|| format!("could not resolve `{}`", cwd.display()))?;
        if let Some(Component::Prefix(prefix)) = canon.components().next() {
            if let Prefix::Disk(d) | Prefix::VerbatimDisk(d) = prefix.kind() {
                return Ok(PathBuf::from(format!("{}:\\", d as char)));
            }
        }
        anyhow::bail!(
            "could not determine the drive root of `{}`; UNC paths are not supported",
            cwd.display()
        )
    }
    #[cfg(not(windows))]
    {
        let _ = cwd;
        Ok(PathBuf::from("/"))
    }
}

/// Convert an absolute, canonicalized host path to the POSIX guest path rooted at
/// the preopened root. On Windows this drops the drive/verbatim prefix
/// (`\\?\C:\a\b` -> `/a/b`); on Unix it is effectively identity.
fn to_guest(p: &Path) -> String {
    use std::path::Component;
    let mut out = String::new();
    for c in p.components() {
        match c {
            // Drop the drive/verbatim prefix and the root; we emit '/' ourselves.
            Component::Prefix(_) | Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => out.push_str("/.."),
            Component::Normal(s) => {
                out.push('/');
                out.push_str(&s.to_string_lossy());
            }
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

/// Stage the bundled standard schemas on the same drive/root as `cwd` so they are
/// reachable through the preopened root. Prefers `OUT_DIR` (same drive as the
/// project during a build script, and outside the watched source tree); otherwise
/// falls back to a temp dir guaranteed to be on the right drive.
fn extract_std_schemas(cwd: &Path) -> Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("capnpc-embedded-std-");
    let dir = match std::env::var_os("OUT_DIR") {
        Some(out) => builder.tempdir_in(out)?,
        // On Windows the system temp dir may be on a different drive than the
        // preopened root; keep staging on `cwd`'s drive.
        None if cfg!(windows) => builder.tempdir_in(cwd)?,
        None => builder.tempdir()?,
    };
    for (rel, bytes) in STD_SCHEMAS {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes)?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::to_guest;
    use std::path::Path;

    #[test]
    #[cfg(not(windows))]
    fn to_guest_unix() {
        assert_eq!(to_guest(Path::new("/a/b/c.capnp")), "/a/b/c.capnp");
        assert_eq!(to_guest(Path::new("/")), "/");
    }

    #[test]
    #[cfg(windows)]
    fn to_guest_windows() {
        // Verbatim (canonicalized) and plain drive paths both drop the prefix.
        assert_eq!(to_guest(Path::new(r"\\?\C:\a\b\c.capnp")), "/a/b/c.capnp");
        assert_eq!(to_guest(Path::new(r"C:\a\b")), "/a/b");
        assert_eq!(to_guest(Path::new(r"C:\")), "/");
    }
}
