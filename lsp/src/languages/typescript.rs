use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tower_lsp::lsp_types::*;
use async_trait::async_trait;
use anyhow::{Result, Context};
use serde_json::{json, Value};
use crate::bazel::BuildGraph;
use super::base_proxy::LspConnection;
use super::coordinator::LanguageServerProxy;

pub struct TypeScriptProxy {
    workspace_root: PathBuf,
    build_graph: Arc<RwLock<BuildGraph>>,
    connection: Arc<Mutex<Option<LspConnection>>>,
}

impl TypeScriptProxy {
    pub fn new(workspace_root: PathBuf, build_graph: Arc<RwLock<BuildGraph>>) -> Self {
        Self {
            workspace_root,
            build_graph,
            connection: Arc::new(Mutex::new(None)),
        }
    }

    async fn ensure_started(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;
        if conn.is_none() {
            // Find TypeScript language server
            let ts_server_path = self.find_typescript_server()
                .context("TypeScript language server not found")?;

            // Configure for Bazel
            let init_options = json!({
                "preferences": {
                    "importModuleSpecifierPreference": "relative",
                    "includePackageJsonAutoImports": "off"
                },
                "tsserver": {
                    "trace": "off"
                }
            });

            let lsp_conn = LspConnection::new(
                ts_server_path.to_str().unwrap(),
                &["--stdio"],
                Some(init_options),
            ).await?;

            // Configure TypeScript for Bazel
            self.configure_typescript(&lsp_conn).await?;

            *conn = Some(lsp_conn);
        }
        Ok(())
    }

    fn find_typescript_server(&self) -> Result<PathBuf> {
        // Try common locations
        let candidates = vec![
            // Global npm install
            which::which("typescript-language-server"),
            // Local node_modules
            Ok(self.workspace_root.join("node_modules/.bin/typescript-language-server")),
            // Common global install paths
            Ok(PathBuf::from("/usr/local/bin/typescript-language-server")),
            Ok(PathBuf::from("/usr/bin/typescript-language-server")),
        ];

        for candidate in candidates {
            if let Ok(path) = candidate {
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        anyhow::bail!("TypeScript language server not found. Install with: npm install -g typescript-language-server")
    }

    async fn configure_typescript(&self, conn: &LspConnection) -> Result<()> {
        // Generate tsconfig.json if not present
        let tsconfig_path = self.workspace_root.join("tsconfig.json");
        if !tsconfig_path.exists() {
            let tsconfig = json!({
                "compilerOptions": {
                    "target": "es2020",
                    "module": "commonjs",
                    "lib": ["es2020"],
                    "strict": true,
                    "esModuleInterop": true,
                    "skipLibCheck": true,
                    "forceConsistentCasingInFileNames": true,
                    "resolveJsonModule": true,
                    "allowJs": true,
                    "checkJs": true,
                    "baseUrl": ".",
                    "paths": {
                        "*": ["*", "bazel-bin/*", "bazel-out/*"]
                    }
                },
                "exclude": [
                    "bazel-*",
                    "node_modules"
                ]
            });
            
            let content = serde_json::to_string_pretty(&tsconfig)?;
            tokio::fs::write(&tsconfig_path, content).await?;
        }

        // Notify about workspace
        conn.notify("workspace/didChangeConfiguration", json!({
            "settings": {}
        })).await?;

        Ok(())
    }

    async fn resolve_bazel_import(&self, import_path: &str) -> Option<PathBuf> {
        // Handle Bazel-generated paths
        if import_path.starts_with("@") {
            // External dependency
            let external_path = self.workspace_root.join("bazel-bin/external");
            let dep_name = import_path.trim_start_matches('@').split('/').next()?;
            let candidate = external_path.join(dep_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        // Check bazel-bin for generated files
        let bazel_bin = self.workspace_root.join(".bazel/bin");
        if bazel_bin.exists() {
            let candidate = bazel_bin.join(import_path);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }
}

#[async_trait]
impl LanguageServerProxy for TypeScriptProxy {
    async fn start(&mut self) -> Result<()> {
        self.ensure_started().await
    }

    async fn shutdown(&mut self) -> Result<()> {
        let mut conn = self.connection.lock().await;
        if let Some(mut lsp_conn) = conn.take() {
            lsp_conn.shutdown().await?;
        }
        Ok(())
    }

    async fn goto_definition(&self, uri: Url, position: Position) -> Result<Option<Location>> {
        self.ensure_started().await?;
        
        let conn = self.connection.lock().await;
        let lsp_conn = conn.as_ref().context("LSP connection not available")?;

        let params = json!({
            "textDocument": { "uri": uri },
            "position": position
        });

        match lsp_conn.request("textDocument/definition", params).await {
            Ok(Value::Array(locations)) => {
                for loc_value in locations {
                    if let Ok(location) = serde_json::from_value::<Location>(loc_value) {
                        return Ok(Some(location));
                    }
                }
                Ok(None)
            }
            Ok(Value::Object(obj)) => {
                Ok(Some(serde_json::from_value::<Location>(Value::Object(obj))?))
            }
            _ => Ok(None)
        }
    }

    async fn completion(&self, uri: Url, position: Position) -> Result<Vec<CompletionItem>> {
        self.ensure_started().await?;
        
        let conn = self.connection.lock().await;
        let lsp_conn = conn.as_ref().context("LSP connection not available")?;

        let params = json!({
            "textDocument": { "uri": uri },
            "position": position,
            "context": {
                "triggerKind": 1,
                "triggerCharacter": "."
            }
        });

        match lsp_conn.request("textDocument/completion", params).await {
            Ok(Value::Array(items)) => {
                let mut completions = Vec::new();
                for item_value in items {
                    if let Ok(item) = serde_json::from_value::<CompletionItem>(item_value) {
                        completions.push(item);
                    }
                }
                Ok(completions)
            }
            Ok(Value::Object(obj)) => {
                if let Some(Value::Array(items)) = obj.get("items") {
                    let mut completions = Vec::new();
                    for item_value in items {
                        if let Ok(item) = serde_json::from_value::<CompletionItem>(item_value.clone()) {
                            completions.push(item);
                        }
                    }
                    Ok(completions)
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(Vec::new())
        }
    }

    async fn hover(&self, uri: Url, position: Position) -> Result<Option<Hover>> {
        self.ensure_started().await?;
        
        let conn = self.connection.lock().await;
        let lsp_conn = conn.as_ref().context("LSP connection not available")?;

        let params = json!({
            "textDocument": { "uri": uri },
            "position": position
        });

        match lsp_conn.request("textDocument/hover", params).await {
            Ok(hover_value) => {
                Ok(serde_json::from_value::<Hover>(hover_value).ok())
            }
            Err(_) => Ok(None)
        }
    }
} 