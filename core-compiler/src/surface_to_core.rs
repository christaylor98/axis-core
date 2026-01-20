// Convert surface AST (as Value) to CoreTerm for evaluation
use crate::runtime::{Value, CoreTerm, Pattern};
use std::rc::Rc;

pub fn value_to_core(v: &Value) -> CoreTerm {
    // Handle CField specially - convert to Proj
    if let Value::Enum(ctor, fields) = v {
        if ctor == "CField" && fields.len() == 2 {
            let obj = Rc::new(value_to_core(&fields[0]));
            let index = if let Value::Int(n) = &fields[1] {
                *n as usize
            } else {
                panic!("CField index must be an Int");
            };
            return CoreTerm::Proj(obj, index, None);
        }
    }
    
    if let Some((ctor_name, ctor_fields)) = try_extract_ctor(v) {
        let lowered_fields = ctor_fields
            .into_iter()
            .map(|field| value_to_core(field))
            .collect();
        return CoreTerm::Ctor(ctor_name, lowered_fields, None);
    }

    match v {
        Value::Int(n) => CoreTerm::IntLit(*n, None),
        Value::Bool(b) => CoreTerm::BoolLit(*b, None),
        Value::Unit => CoreTerm::UnitLit(None),
        Value::Str(handle) => {
            //  Convert string handle to actual string content
            let string_content = crate::get_string(*handle);
            CoreTerm::StrLit(string_content, None)
        }
        Value::Var(name) => CoreTerm::Var(name.clone(), None),
        Value::Lam(param, body) => {
            CoreTerm::Lam(param.clone(), Rc::new(value_to_core(body)), None)
        }
        Value::App(func, arg) => {
            CoreTerm::App(
                Rc::new(value_to_core(func)),
                Rc::new(value_to_core(arg)),
                None
            )
        }
        Value::Let(name, val, body) => {
            CoreTerm::Let(
                name.clone(),
                Rc::new(value_to_core(val)),
                Rc::new(value_to_core(body)),
                None
            )
        }
        Value::If(cond, then_val, else_val) => {
            CoreTerm::If(
                Rc::new(value_to_core(cond)),
                Rc::new(value_to_core(then_val)),
                Rc::new(value_to_core(else_val)),
                None
            )
        }
        Value::Match(scrutinee, arms) => {
            let core_arms: Vec<(Pattern, CoreTerm)> = arms
                .iter()
                .map(|(pattern_str, body)| {
                    let pattern = parse_pattern(pattern_str);
                    let core_body = value_to_core(body);
                    (pattern, core_body)
                })
                .collect();
            CoreTerm::Match(Rc::new(value_to_core(scrutinee)), core_arms, None)
        }
        Value::Tuple(elems) => {
            //  Convert tuple elements
            let core_elems = elems.iter().map(|e| value_to_core(e)).collect();
            CoreTerm::Tuple(core_elems, None)
        }
        _ => panic!("Cannot convert {:?} to CoreTerm", v),
    }
}

// compiler pattern parser - handles constructor patterns with nesting
fn parse_pattern(s: &str) -> Pattern {
    let trimmed = s.trim();
    
    // Try to parse as int literal
    if let Ok(n) = trimmed.parse::<i64>() {
        return Pattern::PInt(n);
    }
    
    // Parse boolean literals
    if trimmed == "true" {
        return Pattern::PBool(true);
    }
    if trimmed == "false" {
        return Pattern::PBool(false);
    }
    
    // Parse unit
    if trimmed == "()" {
        return Pattern::PUnit;
    }
    
    // Parse constructor patterns: Ctor(...) or module.Ctor(...) or Module::Ctor(...)
    // Need to find the LAST opening paren at depth 0 to handle nested patterns
    let chars: Vec<char> = trimmed.chars().collect();
    let mut depth = 0;
    let mut last_top_level_paren: Option<usize> = None;
    
    for (i, &ch) in chars.iter().enumerate() {
        match ch {
            '(' => {
                if depth == 0 {
                    last_top_level_paren = Some(i);
                }
                depth += 1;
            }
            ')' => {
                depth -= 1;
            }
            _ => {}
        }
    }
    
    // If we found a constructor pattern
    if let Some(paren_pos) = last_top_level_paren {
        if let Some(close_paren_pos) = trimmed.rfind(')') {
            // Extract constructor name (everything before opening paren)
            let ctor_part = trimmed[..paren_pos].trim();
            // Remove spaces around . and :: for constructor name
            let ctor_name = ctor_part.replace(" . ", ".").replace(" :: ", "::").replace(" ", "");
            
            // Extract field patterns (everything between parens)
            let fields_str = trimmed[paren_pos + 1..close_paren_pos].trim();
            
            // Split by top-level commas (not inside nested parens)
            let field_patterns: Vec<Pattern> = if fields_str.is_empty() {
                vec![]
            } else {
                split_by_top_level_comma(fields_str)
                    .iter()
                    .map(|f| parse_pattern(f.trim()))
                    .collect()
            };
            
            return Pattern::PEnum(ctor_name, field_patterns);
        }
    }
    
    // Tuple patterns: ( pat1 , pat2 , ... )
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len()-1];
        let elem_patterns: Vec<Pattern> = split_by_top_level_comma(inner)
            .iter()
            .map(|e| parse_pattern(e.trim()))
            .collect();
        return Pattern::PTuple(elem_patterns);
    }
    
    // Check if this is a 0-arity constructor (capitalized identifier with no parens)
    // Examples: Nil, True, False, Ok (when used without arguments)
    if !trimmed.is_empty() && trimmed.chars().next().unwrap().is_uppercase() {
        // Check it's a valid identifier (no spaces, dots, special chars except underscore)
        let is_simple_ident = trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == ':');
        if is_simple_ident {
            // Treat as 0-arity constructor
            return Pattern::PEnum(trimmed.to_string(), vec![]);
        }
    }
    
    // Simple variable binding (including wildcards)
    Pattern::PVar(trimmed.to_string())
}

// Split a string by commas, but only at the top level (not inside parens)
fn split_by_top_level_comma(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                result.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    
    result
}

fn try_extract_ctor<'a>(value: &'a Value) -> Option<(String, Vec<&'a Value>)> {
    match value {
        Value::Var(name) if is_constructor_name(name) => {
            return Some((name.clone(), Vec::new()));
        }
        Value::App(_, _) => {
            let mut args: Vec<&'a Value> = Vec::new();
            let mut current = value;
            while let Value::App(func, arg) = current {
                args.push(arg.as_ref());
                current = func.as_ref();
            }
            if let Value::Var(name) = current {
                if is_constructor_name(name) {
                    args.reverse();
                    return Some((name.clone(), args));
                }
            }
            None
        }
        _ => None,
    }
}

fn is_constructor_name(name: &str) -> bool {
    let last = if let Some(idx) = name.rfind("::") {
        &name[idx + 2..]
    } else if let Some(idx) = name.rfind('.') {
        &name[idx + 1..]
    } else {
        name
    };
    last.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}
