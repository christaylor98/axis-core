// ANDL Loop 6: Semantic Value Runtime for Generated Compiler
// Replaces i64-only fake runtime with proper Value semantics

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Core Value type for the Axis runtime
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(u32),      // String handle into StringTable
    Unit,
    Tuple(Vec<Value>),
    List(Vec<Value>),
    Ctor { tag: u32, fields: Vec<Value> }, // Constructor with tag and fields
}

impl Value {
    pub fn as_int(&self) -> i64 {
        match self {
            Value::Int(n) => *n,
            _ => panic!("Expected Int, got {:?}", self),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            _ => panic!("Expected Bool, got {:?}", self),
        }
    }

    pub fn as_str_handle(&self) -> u32 {
        match self {
            Value::Str(h) => *h,
            _ => panic!("Expected Str, got {:?}", self),
        }
    }

    pub fn as_tuple(&self) -> &Vec<Value> {
        match self {
            Value::Tuple(elems) => elems,
            _ => panic!("Expected Tuple, got {:?}", self),
        }
    }

    pub fn as_list(&self) -> &Vec<Value> {
        match self {
            Value::List(elems) => elems,
            _ => panic!("Expected List, got {:?}", self),
        }
    }

    pub fn as_ctor(&self) -> (u32, &Vec<Value>) {
        match self {
            Value::Ctor { tag, fields } => (*tag, fields),
            _ => panic!("Expected Ctor, got {:?}", self),
        }
    }
}

/// String interning table
pub struct StringTable {
    strings: Vec<String>,
    intern_map: HashMap<String, u32>,
}

impl StringTable {
    pub fn new() -> Self {
        let mut table = StringTable {
            strings: Vec::new(),
            intern_map: HashMap::new(),
        };
        // Reserve handle 0 for empty string
        table.intern("".to_string());
        table
    }

    pub fn intern(&mut self, s: String) -> u32 {
        if let Some(&handle) = self.intern_map.get(&s) {
            return handle;
        }
        let handle = self.strings.len() as u32;
        self.strings.push(s.clone());
        self.intern_map.insert(s, handle);
        handle
    }

    pub fn get(&self, handle: u32) -> Option<&String> {
        self.strings.get(handle as usize)
    }
}

/// Constructor tag table for type/constructor name mapping
pub struct TagTable {
    tags: Vec<String>,
    tag_map: HashMap<String, u32>,
}

impl TagTable {
    pub fn new() -> Self {
        let mut table = TagTable {
            tags: Vec::new(),
            tag_map: HashMap::new(),
        };
        // Pre-register common constructor tags
        table.register("Unit");
        table.register("Nil");
        table.register("Cons");
        table.register("Ok");
        table.register("Err");
        table.register("Some");
        table.register("None");
        table
    }

    pub fn register(&mut self, tag_name: &str) -> u32 {
        if let Some(&tag) = self.tag_map.get(tag_name) {
            return tag;
        }
        let tag = self.tags.len() as u32;
        self.tags.push(tag_name.to_string());
        self.tag_map.insert(tag_name.to_string(), tag);
        tag
    }

    pub fn get_name(&self, tag: u32) -> Option<&String> {
        self.tags.get(tag as usize)
    }

    pub fn get_tag(&self, name: &str) -> Option<u32> {
        self.tag_map.get(name).copied()
    }
}

// Global runtime state using safe patterns
static STRING_TABLE: OnceLock<Mutex<StringTable>> = OnceLock::new();
static TAG_TABLE: OnceLock<Mutex<TagTable>> = OnceLock::new();

/// Initialize the runtime tables
pub fn init_runtime() {
    STRING_TABLE.get_or_init(|| Mutex::new(StringTable::new()));
    TAG_TABLE.get_or_init(|| Mutex::new(TagTable::new()));
}

/// String table operations
pub fn intern_str(s: &str) -> u32 {
    let table = STRING_TABLE.get_or_init(|| Mutex::new(StringTable::new()));
    let mut table = table.lock().unwrap();
    table.intern(s.to_string())
}

pub fn get_str(handle: u32) -> String {
    let table = STRING_TABLE.get_or_init(|| Mutex::new(StringTable::new()));
    let table = table.lock().unwrap();
    table.get(handle).cloned().unwrap_or_else(|| "".to_string())
}

/// Tag table operations  
pub fn register_tag(name: &str) -> u32 {
    let table = TAG_TABLE.get_or_init(|| Mutex::new(TagTable::new()));
    let mut table = table.lock().unwrap();
    table.register(name)
}

pub fn get_tag_name(tag: u32) -> String {
    let table = TAG_TABLE.get_or_init(|| Mutex::new(TagTable::new()));
    let table = table.lock().unwrap();
    table.get_name(tag).cloned().unwrap_or_else(|| "Unknown".to_string())
}

/// Truthiness test for conditionals
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Str(h) => *h != 0, // Empty string (handle 0) is falsy
        Value::Unit => false,
        Value::Tuple(elems) => !elems.is_empty(),
        Value::List(elems) => !elems.is_empty(),
        Value::Ctor { .. } => true, // All constructors are truthy
    }
}