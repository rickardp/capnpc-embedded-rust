fn main() {
    // 1. Run the embedded capnp compiler (no system capnp, no C/C++ toolchain).
    let request = capnpc_embedded::CompileCommand::new()
        .file("schema/addressbook.capnp")
        .src_prefix("schema")
        .compile()
        .expect("failed to compile capnp schema");

    // 2. Generate Rust with our own chosen capnpc version.
    capnpc::codegen::CodeGenerationCommand::new()
        .output_directory(std::env::var("OUT_DIR").unwrap())
        .run(&request[..])
        .expect("failed to generate code");

    println!("cargo:rerun-if-changed=schema/addressbook.capnp");
}
