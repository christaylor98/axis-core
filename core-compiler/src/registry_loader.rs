// Registry loading and parsing for CP-5
// Implements the .axreg file format from axis-registry-0.1.md

use std::collections::HashMap;
use std::fs;

#[allow(dead_code)]
// Registry entries are loaded eagerly but selectively consumed
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub name: String,
    pub arity: u32,
    pub deterministic: bool,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub entries: HashMap<String, RegistryEntry>,
}

#[allow(dead_code)]
// Call classification reserved for effect / foreign-call validation
#[derive(Debug, Clone)]
pub enum CallKind {
    Builtin,    // Hardcoded builtins like arithmetic
    Foreign,    // Registry-based foreign function 
    UserDefined, // User-defined function in program
}

#[allow(dead_code)]
impl Registry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    // Load and parse a single .axreg file
    pub fn load_from_file(&mut self, file_path: &str) -> Result<(), String> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read registry file {}: {}", file_path, e))?;
        
        self.parse_registry_content(&content, file_path)
    }

    // Load multiple registry files
    pub fn load_from_files(&mut self, file_paths: &[&str]) -> Result<(), String> {
        for path in file_paths {
            self.load_from_file(path)?;
        }
        Ok(())
    }

    // Parse .axreg file content according to axis-registry-0.1.md spec
    fn parse_registry_content(&mut self, content: &str, file_path: &str) -> Result<(), String> {
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            
            // Skip empty lines and comments (//)
            if line.is_empty() || line.starts_with("//") {
                i += 1;
                continue;
            }

            // Parse function block
            if line.starts_with("fn ") {
                let name = line[3..].trim().to_string();
                i += 1;

                let mut arity = None;
                let mut deterministic = None;
                let mut profiles = Vec::new();

                // Parse fields until "end"
                while i < lines.len() {
                    let field_line = lines[i].trim();
                    i += 1;

                    if field_line == "end" {
                        break;
                    }

                    if field_line.starts_with("arity ") {
                        let arity_str = field_line[6..].trim();
                        arity = Some(arity_str.parse::<u32>()
                            .map_err(|_| format!("Invalid arity in {}: {}", file_path, arity_str))?);
                    } else if field_line.starts_with("deterministic ") {
                        let det_str = field_line[14..].trim();
                        deterministic = Some(det_str == "true");
                    } else if field_line.starts_with("profile ") {
                        profiles.push(field_line[8..].trim().to_string());
                    }
                }

                // Validate required fields
                let arity = arity.ok_or_else(|| format!("Missing 'arity' field for function '{}' in {}", name, file_path))?;
                let deterministic = deterministic.ok_or_else(|| format!("Missing 'deterministic' field for function '{}' in {}", name, file_path))?;

                // Check for duplicate names (required by spec)
                if self.entries.contains_key(&name) {
                    return Err(format!("Duplicate function name '{}' in registry", name));
                }

                self.entries.insert(name.clone(), RegistryEntry {
                    name,
                    arity,
                    deterministic,
                    profiles,
                });
            } else {
                return Err(format!("Unexpected line in {}: {}", file_path, line));
            }
        }

        Ok(())
    }

    // Single canonical classification function (CP-5 Deliverable A)
    pub fn classify_call(&self, name: &str, arity: u32) -> CallKind {
        // Check builtins first (hardcoded allowlist)
        if is_builtin_function(name) {
            return CallKind::Builtin;
        }

        // Check registry
        if let Some(entry) = self.entries.get(name) {
            if entry.arity == arity {
                return CallKind::Foreign;
            }
            // Arity mismatch is still Foreign (will fail during validation)
            return CallKind::Foreign;
        }

        // Default to UserDefined (will be validated later)
        CallKind::UserDefined
    }

    // Check if a foreign function exists in registry with correct arity
    pub fn validate_foreign_call(&self, name: &str, arity: u32) -> Result<&RegistryEntry, String> {
        match self.entries.get(name) {
            Some(entry) => {
                if entry.arity == arity {
                    Ok(entry)
                } else {
                    Err(format!("Arity mismatch for '{}': expected {}, got {}", name, entry.arity, arity))
                }
            },
            None => Err(format!("Foreign function '{}' not found in registry", name)),
        }
    }


}

// Hardcoded builtin allowlist (as per CP-5 spec)
fn is_builtin_function(name: &str) -> bool {
    match name {
        // Core arithmetic
        "+" | "-" | "*" | "/" | "%" => true,
        "__add__" | "__sub__" | "__mul__" | "__div__" | "__mod__" => true,
        
        // Comparisons  
        "==" | "!=" | "<" | "<=" | ">" | ">=" => true,
        "__eq__" | "__lt__" | "__lte__" | "__gt__" | "__gte__" => true,
        
        // Logical
        "&&" | "||" | "!" => true,
        "__and__" | "__or__" | "__not__" => true,
        
        // Tuple operations
        "tuple_field" => true,
        
        // String operations
        "str_len" | "str_char" | "str_slice" | "str_concat" | "__concat__" => true,
        "int_to_str" | "str_to_int" => true,
        
        // Any function that ends with builtin pattern
        _ if name.ends_with("___main") => true,
        _ if name.contains("__add__") || name.contains("__sub__") => true,
        _ if name.contains("__mul__") || name.contains("__div__") => true,
        
        _ => false,
    }
}