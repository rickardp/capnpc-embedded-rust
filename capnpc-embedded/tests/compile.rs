// Integration test: compile a schema (that imports /capnp/rust.capnp) purely via
// the embedded wasm compiler, then confirm the generated Rust is present.
#[test]
fn compiles_schema_with_rust_annotations() {
    let out = tempfile::tempdir().unwrap();
    capnpc_embedded::CompileCommand::new()
        .file("tests/schema/point.capnp")
        .src_prefix("tests/schema")
        .output_path(out.path())
        .run()
        .expect("compilation failed");

    let generated = out.path().join("point_capnp.rs");
    let src = std::fs::read_to_string(&generated)
        .unwrap_or_else(|e| panic!("expected {}: {e}", generated.display()));
    assert!(src.contains("pub mod point"), "missing point module");
    assert!(src.contains("pub mod line"), "missing line module");
}

// Integration test: compile a schema that uses a *relative* import of a sibling
// schema (plus the bundled `/capnp/rust.capnp`). Exercises cross-platform path
// resolution — the part most likely to differ between Unix and Windows.
#[test]
fn compiles_schema_with_relative_import() {
    let out = tempfile::tempdir().unwrap();
    capnpc_embedded::CompileCommand::new()
        .file("tests/schema/shapes.capnp")
        .file("tests/schema/types.capnp")
        .src_prefix("tests/schema")
        .output_path(out.path())
        .run()
        .expect("compilation failed");

    let shapes = std::fs::read_to_string(out.path().join("shapes_capnp.rs"))
        .expect("shapes_capnp.rs should be generated");
    assert!(shapes.contains("pub mod circle"), "missing circle module");

    let types = std::fs::read_to_string(out.path().join("types_capnp.rs"))
        .expect("types_capnp.rs should be generated");
    assert!(types.contains("pub mod color"), "missing color module");
}

// Sanity: a schema with a genuine error must surface as an Err, not a panic or a
// silently-empty output. Verifies error propagation on every platform.
#[test]
fn reports_schema_error() {
    let out = tempfile::tempdir().unwrap();
    // Stage the input schema on the crate's drive so it is reachable through the
    // preopened root on Windows (system temp may be on another drive).
    let input_dir = tempfile::Builder::new()
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let bad = input_dir.path().join("bad.capnp");
    std::fs::write(&bad, "@0xd7f8e9a0b1c2d3e4;\nstruct Broken { x @0 : NoSuchType; }\n").unwrap();

    let result = capnpc_embedded::CompileCommand::new()
        .file(&bad)
        .output_path(out.path())
        .run();
    assert!(result.is_err(), "expected a compile error, got: {result:?}");
}
