// Minimal surface parser for Batch A
// Parses: let, blocks, calls, literals, identifiers
// Does NOT parse: if, match, operators, lambdas, etc.

use crate::registry_loader::Registry;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub found: String,
    pub expected: String,
    pub source_line: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Parse error in {}:{}:{}",
            self.file, self.line, self.column
        )?;
        writeln!(f, "    {}", self.source_line)?;
        writeln!(f, "    {}^", " ".repeat(self.column.saturating_sub(1)))?;
        write!(f, "Expected '{}', got '{}'", self.expected, self.found)
    }
}

#[allow(dead_code)]
// Byte offsets reserved for future diagnostics
#[derive(Debug, Clone)]
struct SourceLocation {
    line: usize,
    column: usize,
    byte_offset: usize,
}

#[derive(Debug, Clone)]
struct Token {
    text: String,
    location: SourceLocation,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    source: String,
    file: String,
    #[allow(dead_code)]
// Registry retained for future surface-level validation
    registry: Registry,
}

// Unescape string literals: convert \n, \t, \\, \" etc. to actual characters
fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    let mut char_count = 0;
    let mut last_logged = 0;

    while let Some(c) = chars.next() {
        char_count += 1;
        if char_count >= last_logged + 10000 {
            eprintln!("[PROGRESS] phase=axis_compiler loop=unescape count={}", char_count);
            last_logged = char_count;
        }
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('0') => result.push('\0'),
                Some(other) => {
                    // Unknown escape sequence - keep as is
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[derive(Debug, Clone)]
pub enum SurfaceExpr {
    IntLit(i64),
    BoolLit(bool),
    StringLit(String),
    UnitLit,
    Ident(String),
    Proj(Box<SurfaceExpr>, i64),
    Call(String, Vec<SurfaceExpr>),
    Block(Vec<SurfaceStmt>),
    Match(Box<SurfaceExpr>, Vec<MatchArm>),
    If {
        cond: Box<SurfaceExpr>,
        then_branch: Box<SurfaceExpr>,
        else_branch: Box<SurfaceExpr>,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: String,
    pub expr: SurfaceExpr,
}

#[derive(Debug, Clone)]
pub enum SurfaceStmt {
    Let(String, SurfaceExpr),
    LetPattern(String, Vec<String>, SurfaceExpr), // LetPattern(ctor_name, field_vars, rhs)
    Expr(SurfaceExpr),
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: SurfaceExpr,
}
#[allow(dead_code)]
// Foreign function metadata (validated later)
#[derive(Debug, Clone)]
pub struct ForeignFnDef {
    pub name: String,
    pub params: Vec<String>,
    // No body for foreign functions - they're implemented externally
}

#[allow(dead_code)]
// Foreign function metadata (validated later)
// REGIME COMPLIANCE: No modules, no use statements
#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<FnDef>,
    pub foreign_functions: Vec<ForeignFnDef>,
}

pub fn parse_module_with_file(source: &str, file: &str) -> Result<Module, ParseError> {
    // Load registry files for foreign function resolution
    let mut registry = Registry::new();
    let registry_files = [
        "axis/registry/axis.axreg",
        "axis/registry/local.axreg",
    ];

    for registry_file in &registry_files {
        if std::path::Path::new(registry_file).exists() {
            if let Err(e) = registry.load_from_file(registry_file) {
                eprintln!("[WARN] Failed to load registry {}: {}", registry_file, e);
            }
        }
    }

    let tokens = tokenize_with_location(source);
    let mut parser = Parser {
        tokens,
        pos: 0,
        source: source.to_string(),
        file: file.to_string(),
        registry,
    };
    parser.parse_module()
}

#[allow(dead_code)]
// Alternate module entrypoint (unused in compiler)
pub fn parse_module(source: &str) -> Result<Module, String> {
    match parse_module_with_file(source, "<unknown>") {
        Ok(module) => Ok(module),
        Err(err) => Err(err.to_string()),
    }
}

impl Parser {
    fn parse_module(&mut self) -> Result<Module, ParseError> {
        // REGIME COMPLIANCE: No module blocks, no use declarations
        let mut functions = Vec::new();
        let mut foreign_functions = Vec::new();

        while self.pos < self.tokens.len() {
            // Skip comments that became tokens
            while self.pos < self.tokens.len() && self.tokens[self.pos].text.starts_with("//") {
                self.pos += 1;
            }
            if self.pos >= self.tokens.len() {
                break;
            }

            // REGIME COMPLIANCE: Skip use and module keywords (legacy compatibility)
            if self.tokens[self.pos].text == "use" {
                // Skip use declarations silently for backward compatibility
                self.pos += 1;
                let _ = self.parse_path()?;
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == ";" {
                    self.pos += 1;
                }
                continue;
            } else if self.tokens[self.pos].text == "module" {
                // Skip module blocks entirely
                self.skip_module_block()?;
                continue;
            } else if self.tokens[self.pos].text == "type" {
                // Skip type declarations (ADTs)
                self.skip_type_declaration()?;
            } else if self.tokens[self.pos].text == "foreign" {
                // Parse foreign function declaration
                foreign_functions.push(self.parse_foreign_declaration()?);
            } else if self.tokens[self.pos].text == "fn" {
                functions.push(self.parse_function()?);
            } else {
                return self.error(
                    "function, type, or foreign declaration",
                    &self.tokens[self.pos].text,
                );
            }
        }

        Ok(Module {
            functions,
            foreign_functions,
        })
    }

    // REGIME COMPLIANCE: Skip module blocks (backward compatibility only)
    fn skip_module_block(&mut self) -> Result<(), ParseError> {
        self.expect_token("module")?;
        let _ = self.parse_path()?;
        self.expect_token("{")?;

        let mut depth = 1;
        while self.pos < self.tokens.len() && depth > 0 {
            if self.tokens[self.pos].text == "{" {
                depth += 1;
            } else if self.tokens[self.pos].text == "}" {
                depth -= 1;
            }
            self.pos += 1;
        }

        if depth != 0 {
            return self.error("}", "end of input in module block");
        }

        Ok(())
    }

    // Skip a type declaration (ADT)
    // Example: type Foo { Bar(Int), Baz(Str, Bool) }
    fn skip_type_declaration(&mut self) -> Result<(), ParseError> {
        self.expect_token("type")?;

        // Consume type name
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }

        // Skip optional type parameters (e.g., type Foo[T] { ... })
        if self.pos < self.tokens.len() && self.tokens[self.pos].text == "[" {
            self.pos += 1; // consume '['
            while self.pos < self.tokens.len() && self.tokens[self.pos].text != "]" {
                self.pos += 1; // skip type parameter
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                    self.pos += 1;
                }
            }
            self.expect_token("]")?;
        }

        // Expect opening brace
        self.expect_token("{")?;

        // Skip all variants until closing brace
        let mut brace_depth = 1;
        while self.pos < self.tokens.len() && brace_depth > 0 {
            if self.tokens[self.pos].text == "{" {
                brace_depth += 1;
            } else if self.tokens[self.pos].text == "}" {
                brace_depth -= 1;
            }
            self.pos += 1;
        }

        Ok(())
    }

    // Parse a foreign function declaration (CP-5 requirement)
    // Example: foreign fn io_print(msg: Str) -> Unit
    fn parse_foreign_declaration(&mut self) -> Result<ForeignFnDef, ParseError> {
        self.expect_token("foreign")?;
        self.expect_token("fn")?;

        // Get function name (support dotted names like "axis.io.print")
        let name = self.consume_qualified_name()?;

        // Parse parameter list
        self.expect_token("(")?;
        let mut params = Vec::new();
        while self.pos < self.tokens.len() && self.tokens[self.pos].text != ")" {
            params.push(self.consume_token()?.text.clone());
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == ":" {
                self.pos += 1; // skip type annotation colon
                self.skip_type()?; // skip the type expression
            }
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                self.pos += 1;
            }
        }
        self.expect_token(")")?;

        // Skip optional return type
        if self.pos < self.tokens.len() && self.tokens[self.pos].text == "->" {
            self.pos += 1;
            self.skip_type()?;
        }

        Ok(ForeignFnDef { name, params })
    }

    fn parse_function(&mut self) -> Result<FnDef, ParseError> {
        self.expect_token("fn")?;
        let name = self.consume_qualified_name()?;
        self.expect_token("(")?;

        let mut params = Vec::new();
        while self.pos < self.tokens.len() && self.tokens[self.pos].text != ")" {
            params.push(self.consume_token()?.text.clone());
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == ":" {
                self.pos += 1; // skip type annotation colon
                self.skip_type()?; // skip the type expression
            }
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                self.pos += 1;
            }
        }
        self.expect_token(")")?;

        // Skip optional return type
        if self.pos < self.tokens.len() && self.tokens[self.pos].text == "->" {
            self.pos += 1;
            self.skip_type()?; // skip the return type expression
        }

        let body = self.parse_block()?;
        Ok(FnDef { name, params, body })
    }

    // Skip over a type expression in the token stream
    // Handles: Int, Str, qualified.Type, Generic[T], Generic[T1, T2], (T1, T2)
    fn skip_type(&mut self) -> Result<(), ParseError> {
        if self.pos >= self.tokens.len() {
            return self.error("type", "EOF");
        }

        // Handle tuple types (T1, T2, ...)
        if self.tokens[self.pos].text == "(" {
            self.pos += 1; // consume '('

            // Skip tuple element types (can be comma-separated)
            loop {
                if self.pos >= self.tokens.len() {
                    return self.error(")", "EOF");
                }

                if self.tokens[self.pos].text == ")" {
                    self.pos += 1; // consume ')'
                    break;
                }

                // Recursively skip each type in the tuple
                self.skip_type()?;

                // Skip comma if present
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                    self.pos += 1;
                }
            }

            return Ok(());
        }

        // Consume the base type name
        self.pos += 1;

        // Handle qualified types (e.g., parser.SurfaceAst)
        while self.pos < self.tokens.len() && self.tokens[self.pos].text == "." {
            self.pos += 1; // consume '.'
            if self.pos < self.tokens.len() {
                self.pos += 1; // consume type component
            }
        }

        // Handle generic types (e.g., List[Str], Result[Int])
        if self.pos < self.tokens.len() && self.tokens[self.pos].text == "[" {
            self.pos += 1; // consume '['

            // Skip type arguments (can be comma-separated)
            loop {
                if self.pos >= self.tokens.len() {
                    return self.error("]", "EOF");
                }

                if self.tokens[self.pos].text == "]" {
                    self.pos += 1; // consume ']'
                    break;
                }

                // Recursively skip each type argument
                self.skip_type()?;

                // Skip comma if present
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                    self.pos += 1;
                }
            }
        }

        Ok(())
    }

    fn parse_block(&mut self) -> Result<SurfaceExpr, ParseError> {
        self.expect_token("{")?;
        let mut stmts = Vec::new();

        while self.pos < self.tokens.len() && self.tokens[self.pos].text != "}" {
            if self.tokens[self.pos].text == "let" {
                // Peek ahead to see if this is let-in or let-statement
                // Save position
                let saved_pos = self.pos;

                self.pos += 1; // skip 'let'
                if self.pos < self.tokens.len() {
                    self.consume_token().ok(); // skip name
                }

                // Skip optional type annotation
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == ":" {
                    self.pos += 1;
                    // Try to skip type (might fail but that's ok for lookahead)
                    let _ = self.skip_type();
                }

                // Check if we have '=' followed eventually by 'in'
                // For simplicity, we'll just check if after '=' and parsing an expression we see 'in'
                // But actually, let's use a simpler heuristic: scan forward looking for 'in' or ';'
                let mut depth = 0;
                let mut found_in = false;
                let mut _found_semi = false;
                while self.pos < self.tokens.len() {
                    match self.tokens[self.pos].text.as_str() {
                        "{" | "(" | "[" => depth += 1,
                        "}" | ")" | "]" => {
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                        "in" if depth == 0 => {
                            found_in = true;
                            break;
                        }
                        ";" if depth == 0 => {
                            _found_semi = true;
                            break;
                        }
                        _ => {}
                    }
                    self.pos += 1;
                }

                // Restore position
                self.pos = saved_pos;

                if found_in {
                    // This is a let-in expression - parse the whole block as an expression
                    // Actually, let's just parse this as an expression and return it
                    let expr = self.parse_expr()?;
                    self.expect_token("}")?;
                    return Ok(expr);
                } else {
                    // This is a let statement with a pattern
                    self.pos += 1; // consume 'let'

                    // Collect pattern tokens until '='
                    let mut pattern_tokens = Vec::new();
                    while self.pos < self.tokens.len() && self.tokens[self.pos].text != "=" {
                        pattern_tokens.push(self.consume_token()?.text.clone());
                    }

                    self.expect_token("=")?;
                    let expr = self.parse_expr()?;
                    self.expect_token(";")?;

                    //  Detect pattern lets like "Pair(x, y)" or "Ok(val)" or "Token::TokEof(_, _)"
                    // pattern_tokens could be:
                    //   ["Pair", "(", "x", ",", "y", ")"]
                    //   ["Token", "::", "TokEof", "(", "_", ",", "_", ")"]
                    //   ["lexer", "_", "Token", "::", "TokEof", "(", "_", ",", "_", ")"]  (after type annotation)

                    // Find the opening paren to detect pattern let
                    let paren_pos = pattern_tokens.iter().position(|t| t == "(");

                    if let Some(paren_idx) = paren_pos {
                        // Extract constructor name (everything before the paren)
                        // Skip type annotations (tokens ending with ":")
                        let ctor_tokens: Vec<String> = pattern_tokens[..paren_idx]
                            .iter()
                            .filter(|t| !t.ends_with(':'))
                            .cloned()
                            .collect();

                        let ctor_name = ctor_tokens.join("");
                        let mut field_vars = Vec::new();

                        // Extract field names between ( and )
                        let mut i = paren_idx + 1;
                        while i < pattern_tokens.len() && pattern_tokens[i] != ")" {
                            if pattern_tokens[i] != "," {
                                field_vars.push(pattern_tokens[i].clone());
                            }
                            i += 1;
                        }

                        stmts.push(SurfaceStmt::LetPattern(ctor_name, field_vars, expr));
                    } else {
                        // Simple let binding
                        let name = if pattern_tokens.is_empty() {
                            "_".to_string()
                        } else {
                            pattern_tokens[0].clone()
                        };
                        stmts.push(SurfaceStmt::Let(name, expr));
                    }
                }
            } else {
                let expr = self.parse_expr()?;
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == ";" {
                    self.pos += 1;
                    stmts.push(SurfaceStmt::Expr(expr));
                } else {
                    // This is the final expression in the block
                    stmts.push(SurfaceStmt::Expr(expr));
                    // Don't break - let the while loop condition handle the }
                }
            }
        }

        self.expect_token("}")?;
        Ok(SurfaceExpr::Block(stmts))
    }

    fn parse_expr(&mut self) -> Result<SurfaceExpr, ParseError> {
        let mut expr = self.parse_primary_expr()?;

        // Handle binary operators (but not across certain delimiters)
        while self.pos < self.tokens.len() {
            // Stop at delimiters that end expressions
            let tok = &self.tokens[self.pos].text;
            if tok == "," || tok == ";" || tok == ")" || tok == "}" || tok == "]" {
                break;
            }

            let op = tok;
            let op_name = match op.as_str() {
                "++" => "__concat__",
                "+" => "__add__",
                "-" => "__sub__",
                "*" => "__mul__",
                "/" => "__div__",
                "%" => "__mod__",
                "==" => "__eq__",
                "!=" => "__neq__",
                ">=" => "__gte__",
                "<=" => "__lte__",
                ">" => "__gt__",
                "<" => "__lt__",
                "&&" => "__and__",
                "||" => "__or__",
                _ => break,
            };

            self.pos += 1; // consume operator
            let right = self.parse_primary_expr()?;
            expr = SurfaceExpr::Call(op_name.to_string(), vec![expr, right]);
        }

        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> Result<SurfaceExpr, ParseError> {
        if self.pos >= self.tokens.len() {
            return self.error("expression", "EOF");
        }

        if self.tokens[self.pos].text == "let" {
            // let-in expression: let x = expr1 in expr2
            self.pos += 1; // consume 'let'
            let name = self.consume_token()?.text.clone();

            // Optional type annotation
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == ":" {
                self.pos += 1;
                self.skip_type()?;
            }

            self.expect_token("=")?;
            let value = Box::new(self.parse_expr()?);
            self.expect_token("in")?;
            let body = Box::new(self.parse_expr()?);

            // Represent let-in as a special call
            return Ok(SurfaceExpr::Call(
                "__let_in__".to_string(),
                vec![SurfaceExpr::Ident(name), *value, *body],
            ));
        }

        if self.tokens[self.pos].text == "{" {
            return self.parse_block();
        }

        if self.tokens[self.pos].text == "match" {
            return self.parse_match();
        }

        if self.tokens[self.pos].text == "if" {
            return self.parse_if();
        }

        // Reserved keyword: 'fn' is not valid in expression position.
        // Treat as a parse error to prevent it being parsed as an identifier
        // which would later lower to a `Var("fn")` and produce an illegal Core var.
        if self.tokens[self.pos].text == "fn" {
            return self.error("expression", "fn");
        }

        // Unit literal () or tuple expression
        if self.tokens[self.pos].text == "(" {
            let _start_pos = self.pos;
            self.pos += 1; // consume '('

            // Check if this is unit literal ()
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == ")" {
                self.pos += 1; // consume ')'
                return Ok(SurfaceExpr::UnitLit);
            }

            // Otherwise, parse as tuple expression (expr1, expr2, ...)
            // For now, we'll parse it as a call to a special __tuple__ function
            let mut elements = Vec::new();
            loop {
                if self.pos >= self.tokens.len() {
                    return self.error(")", "EOF");
                }

                if self.tokens[self.pos].text == ")" {
                    self.pos += 1; // consume ')'
                    break;
                }

                elements.push(self.parse_expr()?);

                if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                    self.pos += 1;
                } else if self.pos < self.tokens.len() && self.tokens[self.pos].text == ")" {
                    // Allow trailing comma or no comma before )
                    continue;
                } else {
                    return self.error(", or )", &self.tokens[self.pos].text);
                }
            }

            // Represent tuple as a special call
            return Ok(SurfaceExpr::Call("__tuple__".to_string(), elements));
        }

        if self.tokens[self.pos].text.parse::<i64>().is_ok() {
            let token_text = self.consume_token()?.text.clone();
            let n = token_text.parse().unwrap();
            return Ok(SurfaceExpr::IntLit(n));
        }

        // Boolean literals
        if self.tokens[self.pos].text == "true" {
            self.pos += 1;
            return Ok(SurfaceExpr::BoolLit(true));
        }
        if self.tokens[self.pos].text == "false" {
            self.pos += 1;
            return Ok(SurfaceExpr::BoolLit(false));
        }

        // String literals
        if self.tokens[self.pos].text.starts_with('"') && self.tokens[self.pos].text.ends_with('"')
        {
            let token_text = self.consume_token()?.text.clone();
            let content = token_text[1..token_text.len() - 1].to_string(); // Remove quotes
            let unescaped = unescape_string(&content); // Unescape \n, \t, etc.
            return Ok(SurfaceExpr::StringLit(unescaped));
        }

        let mut name = self.consume_token()?.text.clone();

        // Handle qualified identifiers: both '.' and '::'
        // e.g., cli.parse_args or Result::Ok or SurfaceAst::SUnitLit
        loop {
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == "." {
                self.pos += 1;
                if self.pos < self.tokens.len() {
                    let next = self.consume_token()?.text.clone();
                    name = format!("{}.{}", name, next);
                }
            } else if self.pos < self.tokens.len() && self.tokens[self.pos].text == "::" {
                self.pos += 1;
                if self.pos < self.tokens.len() {
                    let next = self.consume_token()?.text.clone();
                    name = format!("{}::{}", name, next);
                }
            } else {
                break;
            }
        }

        if self.pos < self.tokens.len() && self.tokens[self.pos].text == "(" {
            self.pos += 1;
            let mut args = Vec::new();
            while self.pos < self.tokens.len() && self.tokens[self.pos].text != ")" {
                args.push(self.parse_expr()?);
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                    self.pos += 1;
                }
            }
            self.expect_token(")")?;

            // Special-case `proj(expr, index)` per Surface-0.7: index must be
            // a non-negative integer literal and `proj` is a reserved keyword.
            if name == "proj" {
                if args.len() != 2 {
                    return self.error("proj(expr, index)", &format!("proj called with {} args", args.len()));
                }

                // Second arg MUST be an integer literal
                match &args[1] {
                    SurfaceExpr::IntLit(n) if *n >= 0 => {
                        return Ok(SurfaceExpr::Proj(Box::new(args[0].clone()), *n));
                    }
                    _ => {
                        return self.error("proj index (non-negative integer literal)", "non-literal or negative index");
                    }
                }
            }

            Ok(SurfaceExpr::Call(name, args))
        } else if self.pos < self.tokens.len()
            && self.tokens[self.pos].text == "{"
            && name.chars().next().map_or(false, |c| c.is_uppercase())
        {
            // Struct/record literal: TypeName { field1: expr1, field2: expr2, ... }
            // Only if the name starts with uppercase (type name convention)
            self.pos += 1; // consume '{'
            let mut fields = Vec::new();

            while self.pos < self.tokens.len() && self.tokens[self.pos].text != "}" {
                // Parse field_name: expr (per Surface 0.7 spec section 8.1)
                let field_name = self.consume_token()?.text.clone();
                self.expect_token(":")?;
                let field_expr = self.parse_expr()?;

                // Represent the field name as a string literal so lowering
                // treats it as a value (not a variable reference).
                fields.push(SurfaceExpr::StringLit(field_name));
                fields.push(field_expr);

                if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                    self.pos += 1;
                }
            }

            self.expect_token("}")?;

            // Represent struct literal as a special call with type name and field pairs
            let mut all_args = vec![SurfaceExpr::Ident(name)];
            all_args.extend(fields);
            Ok(SurfaceExpr::Call("__struct_lit__".to_string(), all_args))
        } else {
            Ok(SurfaceExpr::Ident(name))
        }
    }

    fn parse_path(&mut self) -> Result<Vec<String>, ParseError> {
        let mut path = vec![self.consume_token()?.text.clone()];
        while self.pos < self.tokens.len() && self.tokens[self.pos].text == "." {
            self.pos += 1;
            path.push(self.consume_token()?.text.clone());
        }
        Ok(path)
    }

    fn parse_match(&mut self) -> Result<SurfaceExpr, ParseError> {
        self.expect_token("match")?;
        let scrutinee = Box::new(self.parse_expr()?);
        self.expect_token("{")?;

        let mut arms = Vec::new();
        while self.pos < self.tokens.len() && self.tokens[self.pos].text != "}" {
            // Parse pattern (simplified - just collect tokens until =>)
            let mut pattern_tokens = Vec::new();
            while self.pos < self.tokens.len()
                && self.tokens[self.pos].text != "=>"
                && self.tokens[self.pos].text != "}"
            {
                pattern_tokens.push(self.consume_token()?.text.clone());
            }

            if pattern_tokens.is_empty() {
                // If we hit }, we're done with arms
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == "}" {
                    break;
                }
                return self.error("pattern", "=>");
            }

            // Join pattern tokens, removing spaces around '.' and '::'
            let mut pattern = String::new();
            let mut i = 0;
            while i < pattern_tokens.len() {
                pattern.push_str(&pattern_tokens[i]);

                // Don't add space before '.', '::', '(', ')', ',', or after '('
                if i + 1 < pattern_tokens.len() {
                    let current = &pattern_tokens[i];
                    let next = &pattern_tokens[i + 1];

                    if next != "."
                        && next != "::"
                        && next != "("
                        && next != ")"
                        && next != ","
                        && current != "("
                        && current != "."
                        && current != "::"
                    {
                        pattern.push(' ');
                    }
                }

                i += 1;
            }

            // Check if we have => or hit end
            if self.pos >= self.tokens.len() || self.tokens[self.pos].text != "=>" {
                // If we don't have =>, treat as end of match
                break;
            }

            self.expect_token("=>")?;

            // Parse the match arm expression
            let expr = self.parse_expr()?;

            arms.push(MatchArm { pattern, expr });

            // Optional comma after the match arm
            if self.pos < self.tokens.len() && self.tokens[self.pos].text == "," {
                self.pos += 1;
            }
        }

        self.expect_token("}")?;
        Ok(SurfaceExpr::Match(scrutinee, arms))
    }

    fn parse_if(&mut self) -> Result<SurfaceExpr, ParseError> {
        self.expect_token("if")?;
        let cond = Box::new(self.parse_expr()?);

        // Parse then block
        self.expect_token("{")?;
        let mut then_stmts = Vec::new();
        while self.pos < self.tokens.len() && self.tokens[self.pos].text != "}" {
            if self.tokens[self.pos].text == "let" {
                self.pos += 1;
                let name = self.consume_token()?.text.clone();
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == ":" {
                    self.pos += 1; // skip ':'
                    self.skip_type()?; // skip the type expression
                }
                self.expect_token("=")?;
                let expr = self.parse_expr()?;
                self.expect_token(";")?;
                then_stmts.push(SurfaceStmt::Let(name, expr));
            } else {
                let expr = self.parse_expr()?;
                if self.pos < self.tokens.len() && self.tokens[self.pos].text == ";" {
                    self.pos += 1;
                    then_stmts.push(SurfaceStmt::Expr(expr));
                } else {
                    then_stmts.push(SurfaceStmt::Expr(expr));
                    break;
                }
            }
        }
        self.expect_token("}")?;

        self.expect_token("else")?;

        // Parse else branch - can be either 'else if' or 'else { ... }'
        let else_branch = if self.pos < self.tokens.len() && self.tokens[self.pos].text == "if" {
            // else if - recursively parse another if expression
            Box::new(self.parse_if()?)
        } else {
            // else block
            self.expect_token("{")?;
            let mut else_stmts = Vec::new();
            while self.pos < self.tokens.len() && self.tokens[self.pos].text != "}" {
                if self.tokens[self.pos].text == "let" {
                    self.pos += 1;
                    let name = self.consume_token()?.text.clone();
                    if self.pos < self.tokens.len() && self.tokens[self.pos].text == ":" {
                        self.pos += 1; // skip ':'
                        self.skip_type()?; // skip the type expression
                    }
                    self.expect_token("=")?;
                    let expr = self.parse_expr()?;
                    self.expect_token(";")?;
                    else_stmts.push(SurfaceStmt::Let(name, expr));
                } else {
                    let expr = self.parse_expr()?;
                    if self.pos < self.tokens.len() && self.tokens[self.pos].text == ";" {
                        self.pos += 1;
                        else_stmts.push(SurfaceStmt::Expr(expr));
                    } else {
                        else_stmts.push(SurfaceStmt::Expr(expr));
                        break;
                    }
                }
            }
            self.expect_token("}")?;
            Box::new(SurfaceExpr::Block(else_stmts))
        };

        Ok(SurfaceExpr::If {
            cond,
            then_branch: Box::new(SurfaceExpr::Block(then_stmts)),
            else_branch,
        })
    }

    fn expect_token(&mut self, expected: &str) -> Result<(), ParseError> {
        if self.pos >= self.tokens.len() {
            return self.error(expected, "EOF");
        }
        let token = &self.tokens[self.pos];
        if token.text == expected {
            self.pos += 1;
            Ok(())
        } else {
            self.error(expected, &token.text)
        }
    }

    fn consume_token(&mut self) -> Result<&Token, ParseError> {
        if self.pos >= self.tokens.len() {
            return self.error("token", "EOF");
        }
        let token = &self.tokens[self.pos];
        self.pos += 1;
        Ok(token)
    }

    // Consume a qualified name (dotted identifier like "axis.char.is_digit")
    // Returns the full name as a string
    fn consume_qualified_name(&mut self) -> Result<String, ParseError> {
        let mut parts = vec![self.consume_token()?.text.clone()];
        
        // Continue consuming "." + identifier pairs
        while self.pos < self.tokens.len() && self.tokens[self.pos].text == "." {
            self.pos += 1; // consume the dot
            if self.pos >= self.tokens.len() {
                return self.error("identifier after '.'", "EOF");
            }
            parts.push(self.consume_token()?.text.clone());
        }
        
        Ok(parts.join("."))
    }

    fn error<T>(&self, expected: &str, found: &str) -> Result<T, ParseError> {
        let (line, column, source_line) = if self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];
            (
                token.location.line,
                token.location.column,
                self.get_source_line(token.location.line),
            )
        } else if let Some(last_token) = self.tokens.last() {
            (
                last_token.location.line,
                last_token.location.column + last_token.text.len(),
                self.get_source_line(last_token.location.line),
            )
        } else {
            (1, 1, "<empty file>".to_string())
        };

        Err(ParseError {
            file: self.file.clone(),
            line,
            column,
            found: found.to_string(),
            expected: expected.to_string(),
            source_line,
        })
    }

    fn get_source_line(&self, line_num: usize) -> String {
        self.source
            .lines()
            .nth(line_num - 1)
            .unwrap_or("<line not found>")
            .to_string()
    }
}

fn tokenize_with_location(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    let mut line = 1;
    let mut column = 1;
    let mut byte_offset = 0;
    let mut last_logged = 0;

    while let Some(&ch) = chars.peek() {
        if byte_offset >= last_logged + 10000 {
            eprintln!("[PROGRESS] phase=axis_compiler loop=tokenize count={}", byte_offset);
            last_logged = byte_offset;
        }
        if ch.is_whitespace() {
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            chars.next();
            byte_offset += 1;
        } else if ch == '/' && chars.clone().nth(1) == Some('/') {
            // Skip line comment
            while let Some(c) = chars.next() {
                byte_offset += 1;
                if c == '\n' {
                    line += 1;
                    column = 1;
                    break;
                }
                column += 1;
            }
        } else if ch.is_alphabetic() || ch == '_' {
            let start_column = column;
            let start_offset = byte_offset;
            let mut ident = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    ident.push(chars.next().unwrap());
                    column += 1;
                    byte_offset += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: ident,
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch.is_numeric() {
            let start_column = column;
            let start_offset = byte_offset;
            let mut num = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_numeric() {
                    num.push(chars.next().unwrap());
                    column += 1;
                    byte_offset += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: num,
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '-' && chars.clone().nth(1) == Some('>') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "->".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '-' && chars.clone().nth(1) == Some('>') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "->".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '+' && chars.clone().nth(1) == Some('+') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "++".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '=' && chars.clone().nth(1) == Some('>') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "=>".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '=' && chars.clone().nth(1) == Some('=') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "==".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '!' && chars.clone().nth(1) == Some('=') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "!=".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '>' && chars.clone().nth(1) == Some('=') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: ">=".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '<' && chars.clone().nth(1) == Some('=') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "<=".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '&' && chars.clone().nth(1) == Some('&') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "&&".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '|' && chars.clone().nth(1) == Some('|') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "||".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == ':' && chars.clone().nth(1) == Some(':') {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next();
            chars.next();
            column += 2;
            byte_offset += 2;
            tokens.push(Token {
                text: "::".to_string(),
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '"' {
            let start_column = column;
            let start_offset = byte_offset;
            chars.next(); // consume opening quote
            column += 1;
            byte_offset += 1;
            let mut string_literal = String::new();
            string_literal.push('"'); // include the opening quote in the token

            while let Some(&c) = chars.peek() {
                if c == '"' {
                    string_literal.push(chars.next().unwrap());
                    column += 1;
                    byte_offset += 1;
                    break;
                } else if c == '\\' {
                    // Handle escape sequences
                    string_literal.push(chars.next().unwrap());
                    column += 1;
                    byte_offset += 1;
                    if let Some(&_next_c) = chars.peek() {
                        string_literal.push(chars.next().unwrap());
                        column += 1;
                        byte_offset += 1;
                    }
                } else {
                    string_literal.push(chars.next().unwrap());
                    if c == '\n' {
                        line += 1;
                        column = 1;
                    } else {
                        column += 1;
                    }
                    byte_offset += 1;
                }
            }

            tokens.push(Token {
                text: string_literal,
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if "(){}[],.;:<>+-*/%".contains(ch) {
            let start_column = column;
            let start_offset = byte_offset;
            let tok = chars.next().unwrap().to_string();
            column += 1;
            byte_offset += 1;
            tokens.push(Token {
                text: tok,
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else if ch == '=' {
            let start_column = column;
            let start_offset = byte_offset;
            let tok = chars.next().unwrap().to_string();
            column += 1;
            byte_offset += 1;
            tokens.push(Token {
                text: tok,
                location: SourceLocation {
                    line,
                    column: start_column,
                    byte_offset: start_offset,
                },
            });
        } else {
            chars.next(); // skip unknown char
            column += 1;
            byte_offset += 1;
        }
    }

    tokens
}

#[allow(dead_code)]
// Keep old tokenize for backward compatibility
fn tokenize(source: &str) -> Vec<String> {
    tokenize_with_location(source)
        .into_iter()
        .map(|t| t.text)
        .collect()
}
