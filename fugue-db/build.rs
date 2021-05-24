fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/fugue_db.capnp")
        .run().expect("schema compiled successfully");
}
