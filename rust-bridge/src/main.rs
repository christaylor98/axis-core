use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use axis_rust_bridge::{core_ir, emit_rust};

// Generated Cap'n Proto schema
mod axis_core_ir_0_1_capnp {
    include!(concat!(env!("OUT_DIR"), "/axis_core_ir_0_1_capnp.rs"));
}

fn usage_and_exit() -> ! {
    eprintln!("Usage:");
    eprintln!("  axis-rust-bridge build <path-to.coreir> --out <binary>");
    eprintln!("  axis-rust-bridge inspect <path-to.coreir>");
    std::process::exit(1)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage_and_exit();
    }

    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("");

    match cmd {
        "inspect" => {
            // Inspect a Core IR file
            if args.len() < 3 {
                eprintln!("Usage: axis-rust-bridge inspect <path-to.coreir>");
                std::process::exit(1);
            }
            let core_path = &args[2];
            match core_ir::inspect_core_bundle(core_path) {
                Ok(summary) => {
                    println!("{}", summary);
                    std::process::exit(0);
                },
                Err(e) => {
                    eprintln!("Failed to inspect Core IR: {}", e);
                    std::process::exit(1);
                }
            }
        },
        "build" => {
            // Build a binary from Core IR
            run_build(&args);
        },
        _ => {
            usage_and_exit();
        }
    }
}

fn run_build(args: &[String]) {
    let phase_start = Instant::now();
    eprintln!("[PHASE] phase4_axis_rust_bridge_run=start");
    
    let exit_code = (|| {
        // Expect: build <path-to.coreir> --out <binary>
        if args.len() < 4 {
            usage_and_exit();
        }

        let core_path = args[2].clone();
        if core_path.starts_with("--") {
            eprintln!("Expected path to .coreir file as first argument");
            usage_and_exit();
        }

        let mut out_path: Option<String> = None;
        let mut i = 3;
        while i < args.len() {
            match args[i].as_str() {
                "--out" => {
                    i += 1;
                    if i >= args.len() {
                        usage_and_exit();
                    }
                    out_path = Some(args[i].clone());
                    i += 1;
                }
                _ => {
                    eprintln!("Unknown arg: {}", args[i]);
                    usage_and_exit();
                }
            }
        }

        if out_path.is_none() {
            usage_and_exit();
        }

        // Prepare temp build directory
        let mut build_dir = env::temp_dir();
        build_dir.push(format!("axis_rust_bridge_build_{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        fs::create_dir_all(build_dir.join("src")).expect("failed to create build dir");

        // 1) Use the provided Core IR file (do not invoke axis-compiler)
        let core_bundle_path = PathBuf::from(core_path);
        if !core_bundle_path.exists() {
            eprintln!("Core IR file not found: {}", core_bundle_path.display());
            return 1;
        }

        // 2) Load Core bundle using core_ir deserialization helper
        let sub_start = Instant::now();
        eprintln!("[PHASE] phase4_core_ir_load=start");
        let core_program = match core_ir::load_core_bundle(core_bundle_path.to_str().unwrap()) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to load Core IR bundle: {}", e);
                return 1;
            }
        };
        eprintln!("[PHASE] phase4_core_ir_load=end ms={}", sub_start.elapsed().as_millis());

    // 3) Emit Rust using existing emitter
    let sub_start = Instant::now();
    eprintln!("[PHASE] phase4_emit_rust=start");
    let generated = emit_rust::emit_rust_from_core(&core_program.root_term, "<core>", "");
    eprintln!("[PHASE] phase4_emit_rust=end ms={}", sub_start.elapsed().as_millis());

    // 4) Write emitted Rust into temporary Cargo package
    // Determine bridge path: if running from release/debug, go up to find axis-rust-bridge
    let bridge_path = {
        let exe_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let mut candidate = exe_path.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();

        // Walk up to find axis-rust-bridge directory
        let mut found = false;
        for _ in 0..5 {
            let test_path = candidate.join("axis-rust-bridge");
            if test_path.join("Cargo.toml").exists() {
                candidate = test_path;
                found = true;
                break;
            }
            if let Some(parent) = candidate.parent() {
                candidate = parent.to_path_buf();
            } else {
                break;
            }
        }

        // Verify we found a valid path
        if !found || !candidate.join("Cargo.toml").exists() {
            candidate = PathBuf::from("/home/chris/dev/axis-lang/axis-rust-bridge"); // Fallback
        }

        candidate
    };

    let emitted_cargo_toml = format!(r#"[package]
name = "axis_emitted"
version = "0.1.0"
edition = "2021"

[dependencies]
axis-rust-bridge = {{ path = "{}" }}
"#, bridge_path.display());
    fs::write(build_dir.join("Cargo.toml"), emitted_cargo_toml).expect("write Cargo.toml");
    // Write generated code into a dedicated module file to avoid emitting
    // an executable `main` here. The bridge will provide the single Rust
    // `main` wrapper that calls `axis_entry`.
    fs::write(build_dir.join("src").join("axis_generated.rs"), generated).expect("write generated Rust");

    // Create a thin executable wrapper that initializes runtime and calls
    // the Axis entry function `axis_entry` produced by the emitter.
    let wrapper = r#"mod axis_generated;
use axis_rust_bridge::runtime::*;

fn main() {
    init_runtime();

    let __phase_start = std::time::Instant::now();

    // Read CLI arguments (skip program name)
    let cli_args: Vec<String> = std::env::args().skip(1).collect();

    // Convert to Axis list representation using Nil/Cons constructors
    let mut axis_args = Value::Ctor {
        tag: intern_tag("Nil"),
        fields: vec![]
    };

    // Build list in reverse order
    for arg in cli_args.iter().rev() {
        axis_args = Value::Ctor {
            tag: intern_tag("Cons"),
            fields: vec![
                Value::Str(intern_str(arg)),
                axis_args
            ]
        };
    }

    // Call Axis entry with arguments
    let result = axis_generated::axis_entry(axis_args);

    // Extract exit code from result
    let exit_code = match result {
        Value::Int(n) => n as i32,
        _ => 0,
    };

    let __phase_elapsed = __phase_start.elapsed().as_millis();

    std::process::exit(exit_code);
}
"#;
    fs::write(build_dir.join("src").join("main.rs"), wrapper).expect("write wrapper main.rs");

        // 5) Invoke cargo build --release in the temp dir
        eprintln!("Building emitted Rust with cargo...");
        let sub_start = Instant::now();
        eprintln!("[PHASE] phase4_cargo_build=start");
        let mut child = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(&build_dir)
            .spawn()
            .expect("failed to spawn cargo build");
        
        // Heartbeat loop: emit progress every 1000ms while cargo runs
        loop {
            match child.try_wait() {
                Ok(Some(_status)) => break,
                Ok(None) => {
                    let elapsed = sub_start.elapsed().as_millis();
                    eprintln!("[PROGRESS] phase=axis_rust_bridge loop=cargo_build_wait count={}", elapsed);
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
                Err(e) => {
                    eprintln!("Error checking cargo status: {}", e);
                    break;
                }
            }
        }
        
        let build_status = child.wait().expect("failed to wait for cargo build");
        eprintln!("[PHASE] phase4_cargo_build=end ms={}", sub_start.elapsed().as_millis());
        if !build_status.success() {
            eprintln!("cargo build failed");
            return 1;
        }

        // 6) Copy resulting binary to --out
        let bin_name = "axis_emitted";
        let built_bin = build_dir.join("target").join("release").join(bin_name);
        let out_path = PathBuf::from(out_path.unwrap());
        fs::copy(&built_bin, &out_path).expect("failed to copy binary to output");

        eprintln!("Wrote binary -> {}", out_path.display());
        0
    })();
    
    eprintln!(
        "[PHASE] phase4_axis_rust_bridge_run=end ms={}",
        phase_start.elapsed().as_millis()
    );
    std::process::exit(exit_code);
}
