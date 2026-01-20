// axis-rust-bridge library

// Generated Cap'n Proto schema
pub mod axis_core_ir_0_1_capnp {
    include!(concat!(env!("OUT_DIR"), "/axis_core_ir_0_1_capnp.rs"));
}

pub mod core_ir;
pub mod core_loader;
pub mod runtime;
pub use runtime::emit_rust;
