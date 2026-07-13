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
