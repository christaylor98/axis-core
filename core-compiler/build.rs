fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("../")
        .file("../axis_core_ir_0_1.capnp")
        .run()
        .expect("schema compiler");
}
