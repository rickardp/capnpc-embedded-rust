// Round-trip integration tests: run the embedded compiler via CompileCommand,
// then generate Rust from the returned CodeGeneratorRequest with a real `capnpc`
// (a dev-dependency) and check the output. This exercises the sidecar workflow a
// consumer would use.

use std::path::Path;

/// Compile the given schema files and generate Rust into `out`.
fn generate(files: &[&str], src_prefix: &str, out: &Path) {
    let mut cmd = capnpc_embedded::CompileCommand::new();
    cmd.src_prefix(src_prefix);
    for f in files {
        cmd.file(f);
    }
    let request = cmd.compile().expect("compilation failed");

    capnpc::codegen::CodeGenerationCommand::new()
        .output_directory(out)
        .run(&request[..])
        .expect("codegen failed");
}

#[test]
fn compiles_schema_with_rust_annotations() {
    let out = tempfile::tempdir().unwrap();
    generate(&["tests/schema/point.capnp"], "tests/schema", out.path());

    let src = std::fs::read_to_string(out.path().join("point_capnp.rs"))
        .expect("point_capnp.rs should be generated");
    assert!(src.contains("pub mod point"), "missing point module");
    assert!(src.contains("pub mod line"), "missing line module");
}

// A *relative* import of a sibling schema (plus the bundled `/capnp/rust.capnp`).
// Exercises cross-platform path resolution — the part most likely to differ
// between Unix and Windows.
#[test]
fn compiles_schema_with_relative_import() {
    let out = tempfile::tempdir().unwrap();
    generate(
        &["tests/schema/shapes.capnp", "tests/schema/types.capnp"],
        "tests/schema",
        out.path(),
    );

    let shapes = std::fs::read_to_string(out.path().join("shapes_capnp.rs"))
        .expect("shapes_capnp.rs should be generated");
    assert!(shapes.contains("pub mod circle"), "missing circle module");

    let types = std::fs::read_to_string(out.path().join("types_capnp.rs"))
        .expect("types_capnp.rs should be generated");
    assert!(types.contains("pub mod color"), "missing color module");
}

// A schema with a genuine error must surface as an Err from `compile()`, not a
// panic or a silently-empty request. Verifies error propagation on every platform.
#[test]
fn reports_schema_error() {
    // Stage the input on the crate's drive so it is reachable through the
    // preopened root on Windows (system temp may be on another drive).
    let input_dir = tempfile::Builder::new()
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let bad = input_dir.path().join("bad.capnp");
    std::fs::write(
        &bad,
        "@0xd7f8e9a0b1c2d3e4;\nstruct Broken { x @0 : NoSuchType; }\n",
    )
    .unwrap();

    let result = capnpc_embedded::CompileCommand::new().file(&bad).compile();
    assert!(result.is_err(), "expected a compile error, got: {result:?}");
}
