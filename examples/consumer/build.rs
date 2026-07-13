fn main() {
    // No system `capnp`, no C/C++ toolchain — the compiler is embedded.
    capnpc_embedded::CompileCommand::new()
        .file("schema/addressbook.capnp")
        .src_prefix("schema")
        .run()
        .expect("failed to compile capnp schema");
    println!("cargo:rerun-if-changed=schema/addressbook.capnp");
}
