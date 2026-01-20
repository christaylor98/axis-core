use clap::{Arg, Command};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

// Generated Cap'n Proto schema
mod axis_core_ir_0_1_capnp {
    include!(concat!(env!("OUT_DIR"), "/axis_core_ir_0_1_capnp.rs"));
}

mod core_loader;
mod core_validator;
// emit_rust module removed for pure Core IR compiler (disabled Rust codegen)
// foreign_impl removed - no runtime execution in compiler
// REGIME COMPLIANCE: module_loader removed (violates rules 7-8)
mod registry_loader;
mod runtime;
mod surface_lower;
mod surface_parser;
mod surface_to_core;
mod trace;
mod validation_registry;

// runtime::Value not used by the Core-IR-only compiler
use registry_loader::Registry;
use trace::trace;

static TRACE_PARSE_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn trace_parse_enabled() -> bool {
    TRACE_PARSE_ENABLED.load(Ordering::Relaxed)
}

static STRING_TABLE: Mutex<Option<StringTable>> = Mutex::new(None);

struct StringTable {
    strings: Vec<String>,
    reverse: HashMap<String, i64>,
}

impl StringTable {
    fn new() -> Self {
        StringTable {
            strings: vec![String::new()],
            reverse: HashMap::new(),
        }
    }

    fn intern(&mut self, s: String) -> i64 {
        if let Some(&handle) = self.reverse.get(&s) {
            return handle;
        }
        let handle = self.strings.len() as i64;
        self.reverse.insert(s.clone(), handle);
        self.strings.push(s);
        handle
    }

    fn get(&self, handle: i64) -> &str {
        if handle < 0 || handle >= self.strings.len() as i64 {
            return "";
        }
        &self.strings[handle as usize]
    }
}

fn init_string_table() {
    let mut table = STRING_TABLE.lock().unwrap();
    if table.is_none() {
        *table = Some(StringTable::new());
    }
}

pub fn intern_string(s: String) -> i64 {
    let mut table = STRING_TABLE.lock().unwrap();
    let table = table.as_mut().unwrap();
    table.intern(s)
}

pub fn get_string(handle: i64) -> String {
    let table = STRING_TABLE.lock().unwrap();
    let table = table.as_ref().unwrap();
    table.get(handle).to_string()
}

fn main() {
    let phase_start = Instant::now();
    eprintln!("[PHASE] phase3_axis_compiler_run=start");
    
    let exit_code = (|| {
        // TRACE: remove after flow is understood
        eprintln!("[TRACE] axis-compiler start");
        
        init_string_table();

        // Parse arguments using Clap
        let matches = Command::new("axis-compiler")
            .about("compiler Axis compiler - produces Core IR bundles from Axis source")
            .arg(
                Arg::new("trace-parse")
                    .long("trace-parse")
                    .help("Enable full parser trace mode")
                    .action(clap::ArgAction::SetTrue)
                    .global(true),
            )
            .arg(
                Arg::new("sources")
                    .short('s')
                    .long("sources")
                    .help("Input Axis source files (concatenated in order)")
                    .num_args(1..)
                    .value_name("FILES"),
            )
            .arg(
                Arg::new("registries")
                    .short('r')
                    .long("registries")
                    .help("Registry files to load (.axreg) in order")
                    .num_args(1..)
                    .value_name("REGS"),
            )
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("out")
                    .help("Output file path for Core IR bundle (default: ./coreir/<source>.coreir)")
                    .value_name("FILE"),
            )
            .arg(
                Arg::new("view-core-ir")
                    .long("view-core-ir")
                    .help("Print textual Core IR graph from a .coreir file and exit")
                    .value_name("FILE")
                    .num_args(1)
                    .conflicts_with_all(["sources", "registries", "output"]),
            )
            .group(
                clap::ArgGroup::new("mode")
                    .args(["sources", "view-core-ir"])
                    .required(true),
            )
            .get_matches();

        // Check if trace-parse is enabled
        if matches.get_flag("trace-parse") {
            TRACE_PARSE_ENABLED.store(true, Ordering::Relaxed);
        }

        // Early exit: --view-core-ir mode
        if let Some(coreir_path) = matches.get_one::<String>("view-core-ir") {
            return match view_core_ir(coreir_path) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    1
                }
            };
        }

        // REGIME COMPLIANCE: Concatenate explicit file list
        let files: Vec<&String> = matches
            .get_many::<String>("sources")
            .unwrap()
            .collect();

        // Registries are required for compilation mode
        let registry_paths: Vec<&String> = match matches.get_many::<String>("registries") {
            Some(regs) => regs.collect(),
            None => {
                eprintln!("Error: --registries is required when compiling sources");
                return 1;
            }
        };

        // REGIME COMPLIANCE: Simple concatenation in the order given
        let mut full_source = String::new();
        for file_path in &files {
            let content = fs::read_to_string(file_path)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to read {}: {}", file_path, e);
                    std::process::exit(1);
                });
            full_source.push_str(&content);
            full_source.push('\n');
        }

        let input_path = files.first().unwrap(); // For diagnostics only

        trace("axis-compiler: loading registries");
        let mut reg = Registry::new();
        let reg_strs: Vec<&str> = registry_paths.iter().map(|s| s.as_str()).collect();
        if let Err(e) = reg.load_from_files(&reg_strs) {
            eprintln!("Failed to load registries: {}", e);
            return 1;
        }

        trace("axis-compiler: parsing and lowering");

        // Parse
        let module = match surface_parser::parse_module_with_file(&full_source, input_path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{}", e);
                return 1;
            }
        };

        // Lower to Core AST (as Value)
        let core_value = surface_lower::lower_module(module);

        // Convert to CoreTerm
        let core_term = surface_to_core::value_to_core(&core_value);

        // Tripwire: ensure axis_io_print is in registry before validation
        assert!(
            reg.entries.contains_key("axis_io_print"),
            "registry missing axis_io_print before validation"
        );

        // Validate Core IR - fail hard on validation error (do not emit bundle)
        // Validation uses the CLI-loaded Registry as the sole authority.
        if let Err(validation_error) = core_validator::validate_core(&core_term, &reg) {
            eprintln!("VALIDATION ERROR: {}", validation_error.message);
            return 1;
        }

        // Create binary core bundle
        let binary_bundle = core_loader::create_core_bundle(&core_term, "main");

        // Determine output path
        let output_path = if let Some(explicit_path) = matches.get_one::<String>("output") {
            // Use explicit path as-is
            explicit_path.clone()
        } else {
            // Default: derive from first source file, output to ./coreir/
            let first_source = files.first().unwrap();
            let source_stem = std::path::Path::new(first_source)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            format!("./coreir/{}.coreir", source_stem)
        };

        // Ensure output directory exists
        if let Some(parent) = std::path::Path::new(&output_path).parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Failed to create output directory: {}", e);
                return 1;
            }
        }

        // Write output
        if let Err(e) = fs::write(&output_path, &binary_bundle) {
            eprintln!("Failed to write output: {}", e);
            return 1;
        }
        eprintln!("Emitted Core bundle -> {}", output_path);
        println!("Axis compiler ready");
        0
    })();
    
    eprintln!(
        "[PHASE] phase3_axis_compiler_run=end ms={}",
        phase_start.elapsed().as_millis()
    );
    std::process::exit(exit_code);
}
#[allow(dead_code)]
// Superseded by --view-core-ir textual graph printer
// Pretty-print Core IR for debugging/proof
fn format_core(term: &runtime::CoreTerm, indent: usize) -> String {
    use runtime::CoreTerm;
    let ind = "  ".repeat(indent);
    match term {
        CoreTerm::IntLit(n, _) => format!("{}IntLit({})", ind, n),
        CoreTerm::BoolLit(b, _) => format!("{}BoolLit({})", ind, b),
        CoreTerm::UnitLit(_) => format!("{}UnitLit", ind),
        CoreTerm::StrLit(s, _) => format!("{}StrLit({:?})", ind, s),
        CoreTerm::Var(name, _) => format!("{}Var({})", ind, name),
        CoreTerm::Ctor(name, fields, _) => {
            let field_strs: Vec<_> = fields.iter().map(|f| format_core(f, indent + 1)).collect();
            if field_strs.is_empty() {
                format!("{}Ctor({})", ind, name)
            } else {
                format!("{}Ctor({}) [\n{}\n{}]", ind, name, field_strs.join(",\n"), ind)
            }
        }
        CoreTerm::Lam(param, body, _) => {
            format!(
                "{}Lam({}) {{\n{}\n{}}}",
                ind,
                param,
                format_core(body, indent + 1),
                ind
            )
        }
        CoreTerm::App(func, arg, _) => {
            format!(
                "{}App(\n{},\n{}\n{})",
                ind,
                format_core(func, indent + 1),
                format_core(arg, indent + 1),
                ind
            )
        }
        CoreTerm::Let(name, val, body, _) => {
            format!(
                "{}Let({}) =\n{}\n{}in\n{}",
                ind,
                name,
                format_core(val, indent + 1),
                ind,
                format_core(body, indent + 1)
            )
        }
        CoreTerm::Tuple(elems, _) => {
            let elem_strs: Vec<_> = elems.iter().map(|e| format_core(e, indent + 1)).collect();
            format!("{}Tuple[\n{}\n{}]", ind, elem_strs.join(",\n"), ind)
        }
        CoreTerm::Proj(tup, idx, _) => {
            format!(
                "{}Proj({},\n{}\n{})",
                ind,
                idx,
                format_core(tup, indent + 1),
                ind
            )
        }
        CoreTerm::If(c, t, e, _) => {
            format!(
                "{}If(\n{},\n{},\n{}\n{})",
                ind,
                format_core(c, indent + 1),
                format_core(t, indent + 1),
                format_core(e, indent + 1),
                ind
            )
        }
        CoreTerm::Match(scrutinee, _, _) => {
            format!(
                "{}Match(\n{}\n{})",
                ind,
                format_core(scrutinee, indent + 1),
                ind
            )
        }
    }
}

// --view-core-ir: Load and print Core IR graph
fn view_core_ir(path: &str) -> Result<(), String> {
    let program = core_loader::load_core_bundle(path)?;
    print_core_bundle(&program);
    Ok(())
}

// Print a textual Core IR graph representation (DAG, not tree)
fn print_core_bundle(program: &core_loader::CoreProgram) {
    use std::collections::HashMap;
    use runtime::CoreTerm;

    // Collect all nodes and assign deterministic IDs
    let mut nodes: Vec<(String, String)> = Vec::new(); // (id, definition)
    let mut node_ids: HashMap<*const CoreTerm, String> = HashMap::new();
    let mut counter = 0usize;

    fn assign_ids(
        term: &CoreTerm,
        nodes: &mut Vec<(String, String)>,
        node_ids: &mut HashMap<*const CoreTerm, String>,
        counter: &mut usize,
    ) -> String {
        use runtime::CoreTerm::*;

        let ptr = term as *const CoreTerm;
        if let Some(id) = node_ids.get(&ptr) {
            return id.clone();
        }

        // Recurse into children first (post-order)
        let (kind, def) = match term {
            IntLit(n, _) => ("literal.int".to_string(), format!("{}", n)),
            BoolLit(b, _) => ("literal.bool".to_string(), format!("{}", b)),
            UnitLit(_) => ("literal.unit".to_string(), String::new()),
            StrLit(s, _) => ("literal.str".to_string(), format!("{:?}", s)),
            Var(name, _) => ("var".to_string(), name.clone()),
            Lam(param, body, _) => {
                let body_id = assign_ids(body, nodes, node_ids, counter);
                ("lam".to_string(), format!("{} -> {}", param, body_id))
            }
            App(func, arg, _) => {
                let func_id = assign_ids(func, nodes, node_ids, counter);
                let arg_id = assign_ids(arg, nodes, node_ids, counter);
                ("app".to_string(), format!("{}({})", func_id, arg_id))
            }
            Tuple(elems, _) => {
                let elem_ids: Vec<String> = elems
                    .iter()
                    .map(|e| assign_ids(e, nodes, node_ids, counter))
                    .collect();
                ("tuple".to_string(), format!("({})", elem_ids.join(", ")))
            }
            Proj(expr, index, _) => {
                let expr_id = assign_ids(expr, nodes, node_ids, counter);
                ("proj".to_string(), format!("{}.{}", expr_id, index))
            }
            Let(name, value, body, _) => {
                let val_id = assign_ids(value, nodes, node_ids, counter);
                let body_id = assign_ids(body, nodes, node_ids, counter);
                ("let".to_string(), format!("{} = {} in {}", name, val_id, body_id))
            }
            If(cond, then_br, else_br, _) => {
                let cond_id = assign_ids(cond, nodes, node_ids, counter);
                let then_id = assign_ids(then_br, nodes, node_ids, counter);
                let else_id = assign_ids(else_br, nodes, node_ids, counter);
                ("if".to_string(), format!("{} ? {} : {}", cond_id, then_id, else_id))
            }
            Ctor(name, fields, _) => {
                let field_ids: Vec<String> = fields
                    .iter()
                    .map(|f| assign_ids(f, nodes, node_ids, counter))
                    .collect();
                if field_ids.is_empty() {
                    ("ctor".to_string(), name.clone())
                } else {
                    ("ctor".to_string(), format!("{}({})", name, field_ids.join(", ")))
                }
            }
            Match(scrutinee, arms, _) => {
                let scrut_id = assign_ids(scrutinee, nodes, node_ids, counter);
                let arm_strs: Vec<String> = arms
                    .iter()
                    .map(|(pat, body)| {
                        let body_id = assign_ids(body, nodes, node_ids, counter);
                        format!("{:?} => {}", pat, body_id)
                    })
                    .collect();
                ("match".to_string(), format!("{} {{ {} }}", scrut_id, arm_strs.join("; ")))
            }
        };

        *counter += 1;
        let id = format!("n{}", *counter);
        node_ids.insert(ptr, id.clone());
        nodes.push((id.clone(), format!("{} {}", kind, def)));
        id
    }

    let entry_id = assign_ids(&program.root_term, &mut nodes, &mut node_ids, &mut counter);

    // Print output
    println!("CoreBundle");
    println!();
    println!("Graph:");
    for (id, def) in &nodes {
        println!("  {} = {}", id, def);
    }
    println!();
    println!("Functions:");
    println!("  main:");
    println!("    entry: {}", entry_id);
}
