# BUILD File Parser Implementation

## Pest Grammar for BUILD Files

```pest
// bazel-lsp/src/bazel/build.pest

// Whitespace and comments
WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
COMMENT = _{ "#" ~ (!"\n" ~ ANY)* }

// Basic tokens
identifier = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
string = @{ "\"" ~ (!"\"" ~ ANY)* ~ "\"" | "'" ~ (!"'" ~ ANY)* ~ "'" }
number = @{ "-"? ~ ASCII_DIGIT+ }
boolean = { "True" | "False" }

// Target references
target_ref = @{ 
    ("//" ~ (!":" ~ !WHITESPACE ~ ANY)* ~ ":" ~ identifier) |
    (":" ~ identifier) |
    identifier
}

// Lists
list = { "[" ~ (list_item ~ ("," ~ list_item)*)? ~ ","? ~ "]" }
list_item = { string | target_ref | number | boolean | list | dict | function_call }

// Dictionaries
dict = { "{" ~ (dict_entry ~ ("," ~ dict_entry)*)? ~ ","? ~ "}" }
dict_entry = { string ~ ":" ~ value }

// Values
value = { string | number | boolean | list | dict | function_call | identifier }

// Function calls (rules)
function_call = { identifier ~ "(" ~ (argument ~ ("," ~ argument)*)? ~ ","? ~ ")" }
argument = { (identifier ~ "=" ~ value) | value }

// Load statements
load_stmt = { "load" ~ "(" ~ string ~ ("," ~ string ~ "=" ~ string)* ~ ")" }

// Variable assignment
assignment = { identifier ~ "=" ~ value }

// Top level
statement = { load_stmt | function_call | assignment }
file = { SOI ~ statement* ~ EOI }
```

## Parser Implementation

```rust
// bazel-lsp/src/bazel/parser.rs

use pest::Parser;
use pest_derive::Parser;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Parser)]
#[grammar = "bazel/build.pest"]
pub struct BuildParser;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(i64),
    Boolean(bool),
    List(Vec<Value>),
    Dict(HashMap<String, Value>),
    TargetRef(String),
    Identifier(String),
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub kind: String,
    pub attributes: HashMap<String, Value>,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

pub struct BuildFileParser {
    package_name: String,
}

impl BuildFileParser {
    pub fn new(package_name: String) -> Self {
        Self { package_name }
    }

    pub fn parse(&self, content: &str) -> Result<Vec<Rule>> {
        let pairs = BuildParser::parse(Rule::file, content)?;
        let mut rules = Vec::new();

        for pair in pairs {
            if let Some(rule) = self.parse_statement(pair)? {
                rules.push(rule);
            }
        }

        Ok(rules)
    }

    fn parse_statement(&self, pair: pest::iterators::Pair<Rule>) -> Result<Option<Rule>> {
        match pair.as_rule() {
            Rule::function_call => {
                let location = Location {
                    line: pair.as_span().start_pos().line_col().0,
                    column: pair.as_span().start_pos().line_col().1,
                };
                
                let mut inner = pair.into_inner();
                let kind = inner.next().unwrap().as_str().to_string();
                
                // Only process known Bazel rules
                if !self.is_bazel_rule(&kind) {
                    return Ok(None);
                }
                
                let mut attributes = HashMap::new();
                
                for arg in inner {
                    let mut arg_inner = arg.into_inner();
                    match arg_inner.clone().count() {
                        2 => {
                            // Named argument
                            let name = arg_inner.next().unwrap().as_str();
                            let value = self.parse_value(arg_inner.next().unwrap())?;
                            attributes.insert(name.to_string(), value);
                        }
                        1 => {
                            // Positional argument (usually 'name')
                            let value = self.parse_value(arg_inner.next().unwrap())?;
                            attributes.insert("name".to_string(), value);
                        }
                        _ => {}
                    }
                }
                
                // Extract target name
                let name = match attributes.get("name") {
                    Some(Value::String(n)) => format!("//{}:{}", self.package_name, n),
                    _ => return Ok(None),
                };
                
                Ok(Some(Rule {
                    name,
                    kind,
                    attributes,
                    location,
                }))
            }
            _ => Ok(None),
        }
    }

    fn parse_value(&self, pair: pest::iterators::Pair<Rule>) -> Result<Value> {
        match pair.as_rule() {
            Rule::string => {
                let s = pair.as_str();
                Ok(Value::String(s[1..s.len()-1].to_string()))
            }
            Rule::number => {
                Ok(Value::Number(pair.as_str().parse()?))
            }
            Rule::boolean => {
                Ok(Value::Boolean(pair.as_str() == "True"))
            }
            Rule::list => {
                let mut items = Vec::new();
                for item in pair.into_inner() {
                    if item.as_rule() == Rule::list_item {
                        items.push(self.parse_value(item.into_inner().next().unwrap())?);
                    }
                }
                Ok(Value::List(items))
            }
            Rule::target_ref => {
                Ok(Value::TargetRef(pair.as_str().to_string()))
            }
            Rule::identifier => {
                Ok(Value::Identifier(pair.as_str().to_string()))
            }
            _ => Ok(Value::String(pair.as_str().to_string())),
        }
    }

    fn is_bazel_rule(&self, name: &str) -> bool {
        matches!(name,
            // Binary rules
            "cc_binary" | "go_binary" | "java_binary" | "py_binary" | "rust_binary" |
            // Library rules
            "cc_library" | "go_library" | "java_library" | "py_library" | "rust_library" |
            // Test rules
            "cc_test" | "go_test" | "java_test" | "py_test" | "rust_test" |
            // Scio custom rules
            "scio_java_test" | "scio_java_junit5_test" | "scio_go_test" |
            // Proto rules
            "proto_library" | "java_proto_library" | "go_proto_library" |
            // Other common rules
            "filegroup" | "genrule" | "test_suite" | "package"
        )
    }
}
```

## Advanced Features

### Incremental Parsing

```rust
// bazel-lsp/src/bazel/incremental.rs

use ropey::Rope;  // Efficient rope data structure for text
use tree_sitter::{Parser, Tree};  // Alternative: tree-sitter for incremental parsing

pub struct IncrementalBuildParser {
    parser: Parser,
    tree: Option<Tree>,
    rope: Rope,
}

impl IncrementalBuildParser {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser.set_language(tree_sitter_starlark::language())?;
        
        Ok(Self {
            parser,
            tree: None,
            rope: Rope::new(),
        })
    }

    pub fn parse_full(&mut self, content: &str) -> Result<Vec<Rule>> {
        self.rope = Rope::from_str(content);
        self.tree = Some(self.parser.parse(content, None)?);
        self.extract_rules()
    }

    pub fn update(&mut self, edit: &TextEdit) -> Result<Vec<Rule>> {
        // Apply edit to rope
        let start_byte = self.rope.line_to_byte(edit.range.start.line) + edit.range.start.character;
        let end_byte = self.rope.line_to_byte(edit.range.end.line) + edit.range.end.character;
        
        self.rope.remove(start_byte..end_byte);
        self.rope.insert(start_byte, &edit.text);
        
        // Re-parse incrementally
        if let Some(old_tree) = &self.tree {
            self.tree = Some(self.parser.parse(
                self.rope.to_string().as_str(),
                Some(old_tree)
            )?);
        }
        
        self.extract_rules()
    }
}
```

### Starlark Evaluation (Optional)

```rust
// bazel-lsp/src/bazel/starlark_eval.rs

use starlark::environment::{Module, Globals};
use starlark::eval::Evaluator;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::Value;

pub struct StarlarkEvaluator {
    globals: Globals,
}

impl StarlarkEvaluator {
    pub fn new() -> Self {
        let mut globals = Globals::standard();
        
        // Add Bazel built-ins
        globals.set("glob", starlark::values::function::NativeFunction::new(
            glob_impl,
            "glob",
            "glob(include, exclude=[])",
        ));
        
        Self { globals }
    }

    pub fn evaluate_build_file(&self, content: &str) -> Result<Module> {
        let ast = AstModule::parse(
            "BUILD",
            content.to_owned(),
            &Dialect::Extended,
        )?;
        
        let module = Module::new();
        let mut eval = Evaluator::new(&module);
        
        eval.eval_module(ast, &self.globals)?;
        
        Ok(module)
    }
}

fn glob_impl<'v>(
    eval: &mut Evaluator<'v, '_>,
    include: Value<'v>,
    exclude: Value<'v>,
) -> anyhow::Result<Value<'v>> {
    // Implement glob functionality
    // This would integrate with the file system to find matching files
    todo!()
}
```

### Caching Layer

```rust
// bazel-lsp/src/bazel/cache.rs

use dashmap::DashMap;
use std::time::{Duration, Instant};
use std::path::PathBuf;
use blake3;  // Fast hashing

#[derive(Clone)]
struct CachedParse {
    rules: Vec<Rule>,
    hash: [u8; 32],
    timestamp: Instant,
}

pub struct ParseCache {
    cache: DashMap<PathBuf, CachedParse>,
    ttl: Duration,
}

impl ParseCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            ttl,
        }
    }

    pub fn get_or_parse<F>(&self, path: &PathBuf, content: &str, parse_fn: F) -> Result<Vec<Rule>>
    where
        F: FnOnce(&str) -> Result<Vec<Rule>>,
    {
        let hash = blake3::hash(content.as_bytes()).as_bytes().clone();
        
        // Check cache
        if let Some(cached) = self.cache.get(path) {
            if cached.hash == hash && cached.timestamp.elapsed() < self.ttl {
                return Ok(cached.rules.clone());
            }
        }
        
        // Parse and cache
        let rules = parse_fn(content)?;
        self.cache.insert(path.clone(), CachedParse {
            rules: rules.clone(),
            hash,
            timestamp: Instant::now(),
        });
        
        Ok(rules)
    }

    pub fn invalidate(&self, path: &PathBuf) {
        self.cache.remove(path);
    }
}
```

## Usage Example

```rust
// Example: Using the parser in the LSP

async fn handle_did_open(&self, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let content = params.text_document.text;
    
    if uri.path().ends_with("BUILD") || uri.path().ends_with("BUILD.bazel") {
        let path = uri.to_file_path().unwrap();
        let package = self.extract_package_name(&path);
        
        let parser = BuildFileParser::new(package);
        match parser.parse(&content) {
            Ok(rules) => {
                // Update build graph
                for rule in rules {
                    self.build_graph.add_target(rule);
                }
                
                // Send diagnostics if needed
                self.validate_rules(&uri, &rules).await;
            }
            Err(e) => {
                // Send parse error as diagnostic
                self.send_diagnostic(uri, e).await;
            }
        }
    }
}
```

## Performance Optimizations

1. **Parallel Parsing**: Use Rayon to parse multiple BUILD files concurrently
2. **Memory Pool**: Reuse allocations for common structures
3. **String Interning**: Intern common strings (rule names, attributes)
4. **Lazy Evaluation**: Only parse what's needed for current operation
5. **Background Indexing**: Parse workspace in background thread

This parser implementation provides the foundation for fast, accurate BUILD file analysis in the VSCode extension.