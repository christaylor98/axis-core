// Dumb module loader: textual concatenation with cycle detection
use std::collections::HashSet;
use std::fs;
use crate::surface_parser;

pub fn load_modules(main_path: &str) -> Result<(String, Vec<String>), String> {
    let mut seen = HashSet::new();
    let mut order: Vec<String> = Vec::new();
    let src = load_recursive(main_path, &mut seen, &mut order)?;
    
    // Write concatenated source to target/axis_concat.ax for inspection
    // Strip 'use' lines from the concatenated output
    let cleaned_src = strip_use_lines(&src);
    if let Err(e) = write_concat_file(&cleaned_src, main_path) {
        eprintln!("[module_loader] Warning: Failed to write concat file: {}", e);
    }
    
    Ok((src, order))
}

fn load_recursive(path: &str, seen: &mut HashSet<String>, order: &mut Vec<String>) -> Result<String, String> {
    if seen.contains(path) {
        return Ok(String::new()); // Already loaded
    }
    seen.insert(path.to_string());
    order.push(path.to_string());
    eprintln!("[module_loader] loaded: {}", path);
    
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path, e))?;
    
    // Parse to extract use declarations
    let module = surface_parser::parse_module_with_file(&source, path)
        .map_err(|e| e.to_string())?;
    
    // Get the directory of the current file to resolve relative imports
    let base_dir = std::path::Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");
    
    // Load dependencies
    let mut deps = String::new();
    for use_path in module.uses {
        let dep_file = resolve_import_path(base_dir, &use_path);
        let dep_source = load_recursive(&dep_file, seen, order)?;
        deps.push_str(&dep_source);
    }
    
    // Return dependencies + this file
    Ok(deps + &source)
}

fn resolve_import_path(base_dir: &str, import_path: &[String]) -> String {
    //  ONLY: Simple resolution relative to importing file's directory
    // Axis module naming: "compiler.cli" when in axis/compiler/ resolves to axis/compiler/cli.ax
    // The first segment typically matches the directory name and should be removed
    if import_path.is_empty() {
        return format!("{}/unknown.ax", base_dir);
    }
    
    let first = &import_path[0];
    
    // Standard library modules: resolve to axis/<full-path>.ax
    if first == "std" || first == "core" || first == "foreign" || first == "runtime" {
        return format!("axis/{}.ax", import_path.join("/"));
    }
    
    // For compiler modules: if first segment matches base directory name, it's a sibling import
    // e.g., from axis/compiler/main.ax, "compiler.cli" -> axis/compiler/cli.ax
    let base_name = std::path::Path::new(base_dir)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    
    if first == base_name && import_path.len() > 1 {
        // Sibling module: use remaining path segments
        format!("{}/{}.ax", base_dir, import_path[1..].join("/"))
    } else {
        // Direct relative path
        format!("{}/{}.ax", base_dir, import_path.join("/"))
    }
}

/// Strip 'use' lines from concatenated source
fn strip_use_lines(source: &str) -> String {
    let without_use = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    
    //  ONLY: Also strip module prefixes from function calls
    // After concatenation, all modules are in one namespace, so:
    // - std.io.println -> println
    // - compiler.compile_file -> compile_file
    // - BUT keep foreign.* as-is (handled specially by compiler)
    strip_module_qualifiers(&without_use)
}

/// Strip non-foreign module qualifiers from concatenated source
/// E.g., "std.io.println" -> "println", "compiler.cli.parse_args" -> "parse_args"
/// BUT "foreign.io.print" stays as "foreign.io.print"
fn strip_module_qualifiers(source: &str) -> String {
    use regex::Regex;
    
    // Match module-qualified names (word.word.identifier)
    // We'll filter out foreign.* in the replacement logic
    let re = Regex::new(r"\b([a-z_][a-z0-9_]*\.)+([a-z_][a-z0-9_A-Z]*)\b").unwrap();
    
    re.replace_all(source, |caps: &regex::Captures| {
        let full_match = caps.get(0).unwrap().as_str();
        // Keep foreign.* unchanged
        if full_match.starts_with("foreign.") {
            full_match.to_string()
        } else {
            // Return just the last segment (the actual function/type name)
            caps.get(2).unwrap().as_str().to_string()
        }
    }).to_string()
}

/// Write concatenated source to target/axis_concat.ax
fn write_concat_file(source: &str, original_path: &str) -> Result<(), String> {
    // Create target directory if it doesn't exist
    fs::create_dir_all("target")
        .map_err(|e| format!("Failed to create target directory: {}", e))?;
    
    let concat_path = "target/axis_concat.ax";
    
    // Prepend a comment with metadata
    let header = format!(
        "//  MODULE CONCATENATION\n\
         // Original root: {}\n\
         // Note: 'use' declarations have been stripped\n\n",
        original_path
    );
    
    let content = header + source;
    
    fs::write(concat_path, content)
        .map_err(|e| format!("Failed to write {}: {}", concat_path, e))?;
    
    eprintln!("[module_loader] Wrote concatenated source to {}", concat_path);
    Ok(())
}
