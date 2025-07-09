# VSCode Bazel Extension - Rust Implementation Design

## Overview

This document outlines an updated architecture using Rust for the core implementation of the VSCode Bazel extension. The design leverages Rust's performance advantages for compute-intensive operations while maintaining TypeScript for VSCode API integration.

## Architecture Changes

### Hybrid Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   VSCode Extension (TypeScript)                  │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │   UI Layer  │  │   Commands   │  │  Extension Activation  │ │
│  │ - TreeViews │  │  - Register  │  │  - Start LSP servers   │ │
│  │ - CodeLens  │  │  - Handle    │  │  - Manage lifecycle    │ │
│  └─────────────┘  └──────────────┘  └────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                    Language Server Protocol                      │
├─────────────────────────────────────────────────────────────────┤
│              Bazel Language Server (Rust + tower-lsp)           │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │Bazel Client │  │ Build Graph  │  │   Language Service     │ │
│  │- Query API  │  │ - Fast parse │  │   Coordinator          │ │
│  │- BEP parser │  │ - Incremental│  │ - Route requests       │ │
│  └─────────────┘  └──────────────┘  └────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                 Language-Specific LSP Proxies                    │
│  ┌──────────┐  ┌──────────────┐  ┌─────────┐  ┌─────────────┐ │
│  │ Go Proxy │  │  TS Proxy    │  │ Python  │  │ Java Proxy  │ │
│  │  (Rust)  │  │   (Rust)     │  │ Proxy   │  │   (Rust)    │ │
│  └──────────┘  └──────────────┘  └─────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                 External Language Servers                        │
│  ┌──────────┐  ┌──────────────┐  ┌─────────┐  ┌─────────────┐ │
│  │  gopls   │  │ TS Language  │  │ pylsp/  │  │    jdtls    │ │
│  │          │  │   Server     │  │ pyright │  │             │ │
│  └──────────┘  └──────────────┘  └─────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Rust Implementation Structure

### Project Layout
```
vscode-bazel-extension/
├── extension/                      # TypeScript VSCode extension
│   ├── src/
│   │   ├── extension.ts
│   │   ├── client.ts              # LSP client setup
│   │   └── commands.ts
│   ├── package.json
│   └── tsconfig.json
├── bazel-lsp/                      # Rust LSP server
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── server.rs              # tower-lsp server impl
│   │   ├── bazel/
│   │   │   ├── mod.rs
│   │   │   ├── client.rs          # Bazel CLI wrapper
│   │   │   ├── query.rs           # Query operations
│   │   │   ├── build_graph.rs     # BUILD file parsing
│   │   │   └── bep.rs             # Build Event Protocol
│   │   ├── languages/
│   │   │   ├── mod.rs
│   │   │   ├── coordinator.rs
│   │   │   ├── go.rs
│   │   │   ├── typescript.rs
│   │   │   ├── python.rs
│   │   │   └── java.rs
│   │   └── cache/
│   │       ├── mod.rs
│   │       └── lru.rs
│   └── tests/
└── scripts/
    ├── build.sh
    └── package.sh
```

## Core Rust Components

### 1. Tower-LSP Server Implementation

```rust
// bazel-lsp/Cargo.toml
[package]
name = "bazel-lsp"
version = "0.1.0"
edition = "2021"

[dependencies]
tower-lsp = "0.20"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
dashmap = "5"  # Concurrent hashmap for caching
pest = "2"     # Parser for BUILD files
pest_derive = "2"
rayon = "1"    # Parallel processing
lru = "0.12"
tracing = "0.1"
tracing-subscriber = "0.3"
which = "6"    # Find bazel executable
tempfile = "3"
walkdir = "2"
anyhow = "1"
thiserror = "1"

# For protobuf parsing (Bazel query output)
prost = "0.12"
prost-build = "0.12"

# For integrating with external LSPs
lsp-types = "0.95"
lsp-server = "0.7"
```

```rust
// bazel-lsp/src/server.rs
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct BazelLanguageServer {
    client: Client,
    build_graph: Arc<RwLock<BuildGraph>>,
    bazel_client: Arc<BazelClient>,
    language_coordinator: Arc<LanguageCoordinator>,
    document_cache: Arc<DashMap<Url, String>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for BazelLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let workspace_root = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        // Initialize build graph in background
        let build_graph = self.build_graph.clone();
        let root = workspace_root.clone();
        tokio::spawn(async move {
            let mut graph = build_graph.write().await;
            if let Err(e) = graph.scan_workspace(&root).await {
                tracing::error!("Failed to scan workspace: {}", e);
            }
        });

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["/".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "bazel.build".to_string(),
                        "bazel.test".to_string(),
                        "bazel.run".to_string(),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Fast path: check if it's a Bazel target reference
        if let Some(target_ref) = self.extract_bazel_target(&uri, position).await {
            if let Some(location) = self.resolve_bazel_target(&target_ref).await {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
            }
        }

        // Delegate to language-specific handler
        self.language_coordinator
            .goto_definition(uri, position)
            .await
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        
        if uri.path().ends_with("BUILD") || uri.path().ends_with("BUILD.bazel") {
            let build_graph = self.build_graph.read().await;
            let lenses = build_graph.get_code_lenses(&uri)?;
            Ok(Some(lenses))
        } else {
            // Check if file belongs to a test target
            let build_graph = self.build_graph.read().await;
            if let Some(target) = build_graph.get_target_for_file(&uri) {
                if target.is_test() {
                    Ok(Some(vec![
                        CodeLens {
                            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                            command: Some(Command {
                                title: "▶️ Run Test".to_string(),
                                command: "bazel.test".to_string(),
                                arguments: Some(vec![serde_json::to_value(&target.label)?]),
                            }),
                            data: None,
                        },
                        CodeLens {
                            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                            command: Some(Command {
                                title: "🐛 Debug Test".to_string(),
                                command: "bazel.debug".to_string(),
                                arguments: Some(vec![serde_json::to_value(&target.label)?]),
                            }),
                            data: None,
                        },
                    ]))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        match params.command.as_str() {
            "bazel.build" => {
                let target = params.arguments.get(0)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
                
                self.bazel_client.build(target).await?;
                Ok(None)
            }
            "bazel.test" => {
                let target = params.arguments.get(0)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
                
                self.bazel_client.test(target).await?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}
```

### 2. High-Performance BUILD File Parser

```rust
// bazel-lsp/src/bazel/build_graph.rs
use pest::Parser;
use pest_derive::Parser;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[grammar = "bazel/build.pest"]
pub struct BuildParser;

#[derive(Debug, Clone)]
pub struct BazelTarget {
    pub label: String,
    pub kind: String,
    pub srcs: Vec<String>,
    pub deps: Vec<String>,
    pub location: Location,
    pub attributes: HashMap<String, Value>,
}

pub struct BuildGraph {
    targets: DashMap<String, BazelTarget>,
    file_to_targets: DashMap<PathBuf, Vec<String>>,
    workspace_root: PathBuf,
}

impl BuildGraph {
    pub async fn scan_workspace(&mut self, root: &Path) -> anyhow::Result<()> {
        let build_files: Vec<_> = WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                (name == "BUILD" || name == "BUILD.bazel") && !e.path().starts_with("bazel-")
            })
            .map(|e| e.path().to_owned())
            .collect();

        // Parse BUILD files in parallel using Rayon
        let results: Vec<_> = build_files
            .par_iter()
            .map(|path| self.parse_build_file(path))
            .collect();

        // Process results
        for result in results {
            if let Err(e) = result {
                tracing::warn!("Failed to parse BUILD file: {}", e);
            }
        }

        Ok(())
    }

    fn parse_build_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)?;
        let pairs = BuildParser::parse(Rule::file, &content)?;

        for pair in pairs {
            if let Some(target) = self.parse_rule(pair, path)? {
                let label = target.label.clone();
                
                // Update file mappings
                for src in &target.srcs {
                    let src_path = path.parent().unwrap().join(src);
                    self.file_to_targets
                        .entry(src_path)
                        .or_insert_with(Vec::new)
                        .push(label.clone());
                }

                self.targets.insert(label, target);
            }
        }

        Ok(())
    }

    pub fn get_target_for_file(&self, file: &Url) -> Option<BazelTarget> {
        let path = file.to_file_path().ok()?;
        let targets = self.file_to_targets.get(&path)?;
        targets.first().and_then(|label| {
            self.targets.get(label).map(|t| t.clone())
        })
    }
}
```

### 3. Bazel Client with Async Operations

```rust
// bazel-lsp/src/bazel/client.rs
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use futures::stream::StreamExt;

pub struct BazelClient {
    workspace_root: PathBuf,
    bazel_path: PathBuf,
    query_cache: Arc<Mutex<LruCache<String, QueryResult>>>,
}

impl BazelClient {
    pub async fn new(workspace_root: PathBuf) -> anyhow::Result<Self> {
        let bazel_path = which::which("bazel")?;
        Ok(Self {
            workspace_root,
            bazel_path,
            query_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(1000).unwrap()
            ))),
        })
    }

    pub async fn query(&self, query: &str) -> anyhow::Result<QueryResult> {
        // Check cache first
        {
            let mut cache = self.query_cache.lock().await;
            if let Some(result) = cache.get(query) {
                return Ok(result.clone());
            }
        }

        let output = Command::new(&self.bazel_path)
            .current_dir(&self.workspace_root)
            .args(&[
                "query",
                query,
                "--output=proto",
                "--proto:output_rule_attrs=srcs,deps,visibility,testonly",
            ])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("Bazel query failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        // Parse protobuf output
        let result = self.parse_query_output(&output.stdout)?;
        
        // Cache result
        {
            let mut cache = self.query_cache.lock().await;
            cache.put(query.to_string(), result.clone());
        }

        Ok(result)
    }

    pub async fn build(&self, target: &str) -> anyhow::Result<BuildResult> {
        let mut child = Command::new(&self.bazel_path)
            .current_dir(&self.workspace_root)
            .args(&["build", target, "--build_event_json_file=-"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Stream build events
        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            // Parse Build Event Protocol JSON
            if let Ok(event) = serde_json::from_str::<BuildEvent>(&line) {
                self.handle_build_event(event).await?;
            }
        }

        let status = child.wait().await?;
        Ok(BuildResult { success: status.success() })
    }
}
```

### 4. Language Coordinator with External LSP Integration

```rust
// bazel-lsp/src/languages/coordinator.rs
use lsp_types::*;
use tokio::net::TcpStream;
use tokio::process::Command;

pub struct LanguageCoordinator {
    workspace_root: PathBuf,
    build_graph: Arc<RwLock<BuildGraph>>,
    language_servers: DashMap<String, Box<dyn LanguageServerProxy>>,
}

#[async_trait]
trait LanguageServerProxy: Send + Sync {
    async fn start(&mut self) -> anyhow::Result<()>;
    async fn goto_definition(&self, uri: Url, position: Position) -> anyhow::Result<Option<Location>>;
    async fn completion(&self, uri: Url, position: Position) -> anyhow::Result<Vec<CompletionItem>>;
}

struct GoProxy {
    client: Option<lsp_server::Connection>,
    build_graph: Arc<RwLock<BuildGraph>>,
}

#[async_trait]
impl LanguageServerProxy for GoProxy {
    async fn start(&mut self) -> anyhow::Result<()> {
        let mut gopls = Command::new("gopls")
            .args(&["-mode=stdio"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        // Configure gopls for Bazel
        let init_params = InitializeParams {
            initialization_options: Some(serde_json::json!({
                "build.directoryFilters": ["-bazel-*"],
                "build.experimentalWorkspaceModule": true,
            })),
            ..Default::default()
        };

        // Set up bidirectional communication
        // ... (connection setup code)

        Ok(())
    }

    async fn goto_definition(&self, uri: Url, position: Position) -> anyhow::Result<Option<Location>> {
        // First try gopls
        if let Some(location) = self.query_gopls(uri.clone(), position).await? {
            return Ok(Some(location));
        }

        // Fall back to Bazel-aware resolution
        let path = uri.to_file_path().map_err(|_| anyhow::anyhow!("Invalid URI"))?;
        let content = tokio::fs::read_to_string(&path).await?;
        
        // Extract import at position
        if let Some(import) = self.extract_import_at_position(&content, position) {
            // Resolve Bazel-style imports
            if import.starts_with("github.com/askscio/scio/") {
                let relative = import.strip_prefix("github.com/askscio/scio/").unwrap();
                let target_path = self.workspace_root.join(relative);
                
                if target_path.exists() {
                    return Ok(Some(Location {
                        uri: Url::from_file_path(target_path).unwrap(),
                        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    }));
                }
            }
        }

        Ok(None)
    }
}
```

## TypeScript Extension Integration

### Extension Entry Point

```typescript
// extension/src/extension.ts
import * as vscode from 'vscode';
import * as path from 'path';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient;

export async function activate(context: vscode.ExtensionContext) {
    // Path to the Rust LSP binary
    const serverModule = context.asAbsolutePath(
        path.join('server', process.platform === 'win32' ? 'bazel-lsp.exe' : 'bazel-lsp')
    );

    const serverOptions: ServerOptions = {
        run: { command: serverModule },
        debug: {
            command: serverModule,
            args: ['--debug'],
            options: { env: { RUST_LOG: 'debug' } }
        }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', pattern: '**/BUILD{,.bazel}' },
            { scheme: 'file', language: 'go' },
            { scheme: 'file', language: 'typescript' },
            { scheme: 'file', language: 'python' },
            { scheme: 'file', language: 'java' }
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/BUILD{,.bazel}')
        }
    };

    client = new LanguageClient(
        'bazel-lsp',
        'Bazel Language Server',
        serverOptions,
        clientOptions
    );

    // Register additional commands
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.showTargetInfo', showTargetInfo),
        vscode.commands.registerCommand('bazel.buildCurrentTarget', buildCurrentTarget),
        vscode.commands.registerCommand('bazel.testCurrentTarget', testCurrentTarget)
    );

    await client.start();
}

async function buildCurrentTarget() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    // Get target from server
    const params = {
        textDocument: { uri: editor.document.uri.toString() }
    };
    
    const target = await client.sendRequest('bazel/getTargetForFile', params);
    if (target) {
        await client.sendRequest('bazel/build', { target });
    }
}
```

## Performance Benefits

### Rust Advantages

1. **BUILD File Parsing**: 10-50x faster than regex-based TypeScript parsing
2. **Parallel Processing**: Rayon enables true parallelism for workspace scanning
3. **Memory Efficiency**: Zero-copy parsing and efficient caching
4. **Query Caching**: Thread-safe LRU cache with minimal overhead
5. **Async I/O**: Tokio enables non-blocking Bazel operations

### Benchmarks (estimated)

| Operation | TypeScript | Rust | Improvement |
|-----------|------------|------|-------------|
| Parse 1000 BUILD files | 5-10s | 200-500ms | 10-50x |
| Bazel query (cached) | 50ms | 2ms | 25x |
| Find target for file | 100ms | 5ms | 20x |
| Workspace indexing | 30s | 2-3s | 10-15x |

## Build and Distribution

### Build Script

```bash
#!/bin/bash
# scripts/build.sh

# Build Rust LSP server
cd bazel-lsp
cargo build --release

# Copy to extension
mkdir -p ../extension/server
cp target/release/bazel-lsp ../extension/server/

# Build extension
cd ../extension
npm install
npm run compile
vsce package
```

### Cross-Platform Support

```toml
# bazel-lsp/Cargo.toml
[profile.release]
opt-level = 3
lto = true
strip = true

# Platform-specific dependencies
[target.'cfg(windows)'.dependencies]
windows = "0.51"

[target.'cfg(unix)'.dependencies]
nix = "0.27"
```

## Migration Path

1. **Phase 1**: Implement core Rust LSP with BUILD file support
2. **Phase 2**: Add Bazel query/build/test commands
3. **Phase 3**: Integrate language-specific proxies
4. **Phase 4**: Optimize caching and performance
5. **Phase 5**: Add advanced features (debugging, refactoring)

## Conclusion

Using Rust with tower-lsp provides significant performance benefits while maintaining full VSCode integration. The hybrid approach allows us to leverage Rust's strengths for compute-intensive operations while keeping the UI layer in TypeScript for easier VSCode API integration.