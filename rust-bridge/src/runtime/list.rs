// List primitives
// Extracted from emit_rust.rs generate_value_runtime()

use super::value::Value;

pub fn list_nil() -> Value {
    Value::List(vec![])
}

pub fn list_cons(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match &elems[1] {
                Value::List(list_elems) => {
                    let mut new_elems = list_elems.clone();
                    new_elems.insert(0, elems[0].clone());
                    Value::List(new_elems)
                },
                _ => Value::List(vec![elems[0].clone()]),
            }
        },
        _ => Value::List(vec![]),
    }
}

pub fn list_reverse(list: Value) -> Value {
    match list {
        Value::List(mut elems) => {
            elems.reverse();
            Value::List(elems)
        },
        _ => Value::List(vec![]),
    }
}

pub fn list_concat(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            match (&elems[0], &elems[1]) {
                (Value::List(list1), Value::List(list2)) => {
                    let mut result = list1.clone();
                    result.extend(list2.clone());
                    Value::List(result)
                },
                (Value::List(list), _) => Value::List(list.clone()),
                (_, Value::List(list)) => Value::List(list.clone()),
                _ => Value::List(vec![]),
            }
        },
        _ => Value::List(vec![]),
    }
}

pub fn list_contains_str(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let needle_str = match &elems[1] {
                Value::Str(handle) => super::value::get_str(*handle),
                _ => return Value::Bool(false),
            };
            
            match &elems[0] {
                Value::List(list_elems) => {
                    for elem in list_elems {
                        if let Value::Str(handle) = elem {
                            if super::value::get_str(*handle) == needle_str {
                                return Value::Bool(true);
                            }
                        }
                    }
                    Value::Bool(false)
                },
                _ => Value::Bool(false),
            }
        },
        _ => Value::Bool(false),
    }
}

pub fn list_index_of_str(args: Value) -> Value {
    match args {
        Value::Tuple(ref elems) if elems.len() >= 2 => {
            let needle_str = match &elems[1] {
                Value::Str(handle) => super::value::get_str(*handle),
                _ => return Value::Int(-1),
            };
            
            match &elems[0] {
                Value::List(list_elems) => {
                    for (i, elem) in list_elems.iter().enumerate() {
                        if let Value::Str(handle) = elem {
                            if super::value::get_str(*handle) == needle_str {
                                return Value::Int(i as i64);
                            }
                        }
                    }
                    Value::Int(-1)
                },
                _ => Value::Int(-1),
            }
        },
        _ => Value::Int(-1),
    }
}
