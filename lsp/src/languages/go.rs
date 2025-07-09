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

pub struct GoProxy {
    workspace_root: PathBuf,
    build_graph: Arc<RwLock<BuildGraph>>,
    connection: Arc<Mutex<Option<LspConnection>>>,
}

impl GoProxy {
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
            // Find gopls
            let gopls_path = which::which("gopls")
                .context("gopls not found. Please install gopls: go install golang.org/x/tools/gopls@latest")?;

            // Configure gopls for Bazel
            let init_options = json!({
                "build.directoryFilters": ["-.bazel/*"],
                "build.experimentalWorkspaceModule": true,
                "formatting.gofumpt": true,
                "ui.semanticTokens": true,
                "ui.completion.usePlaceholders": true,
            });

            let lsp_conn = LspConnection::new(
                gopls_path.to_str().unwrap(),
                &["-mode=stdio"],
                Some(init_options),
            ).await?;

            // Open workspace
            self.open_workspace(&lsp_conn).await?;

            *conn = Some(lsp_conn);
        }
        Ok(())
    }

    async fn open_workspace(&self, conn: &LspConnection) -> Result<()> {
        // Generate go.mod if needed for gopls
        let go_mod_path = self.workspace_root.join("go/go.mod");
        if !go_mod_path.exists() {
            // Create a temporary go.mod for gopls
            let module_name = self.guess_module_name().await;
            let go_mod_content = format!(
                "module {}\n\ngo 1.20\n",
                module_name
            );
            tokio::fs::write(&go_mod_path, go_mod_content).await?;
        }

        // Notify gopls about workspace folders
        conn.notify("workspace/didChangeWorkspaceFolders", json!({
            "event": {
                "added": [{
                    "uri": Url::from_file_path(&self.workspace_root).unwrap(),
                    "name": self.workspace_root.file_name().unwrap().to_str().unwrap()
                }],
                "removed": []
            }
        })).await?;

        Ok(())
    }

    async fn guess_module_name(&self) -> String {
        // Try to guess module name from Bazel workspace
        if let Ok(content) = tokio::fs::read_to_string(self.workspace_root.join("WORKSPACE")).await {
            // Look for go_repository rules
            if let Some(line) = content.lines().find(|l| l.contains("github.com/")) {
                if let Some(module) = line.split('"').find(|s| s.starts_with("github.com/")) {
                    return module.to_string();
                }
            }
        }
        
        // Default to directory name
        self.workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string()
    }

    async fn translate_import_path(&self, import_path: &str) -> Option<PathBuf> {
        // Handle Bazel-style imports
        if import_path.starts_with("github.com/") || import_path.contains('/') {
            // Check if this is our workspace module
            let module_name = self.guess_module_name().await;
            if import_path.starts_with(&module_name) {
                let relative = import_path.strip_prefix(&module_name)
                    .unwrap_or(import_path)
                    .trim_start_matches('/');
                return Some(self.workspace_root.join(relative));
            }
            
            // Check Bazel's external directory
            let external_path = self.workspace_root.join(".bazel/bin/external");
            if external_path.exists() {
                let parts: Vec<&str> = import_path.split('/').collect();
                if parts.len() >= 3 {
                    let repo = parts[..3].join("/");
                    let rest = parts[3..].join("/");
                    let candidate = external_path.join(&repo).join(&rest);
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
        
        None
    }
}

#[async_trait]
impl LanguageServerProxy for GoProxy {
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
                // Try to resolve Bazel imports
                for loc_value in locations {
                    if let Ok(location) = serde_json::from_value::<Location>(loc_value) {
                        return Ok(Some(location));
                    }
                }
                Ok(None)
            }
            Ok(Value::Object(obj)) => {
                // Single location
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
                "triggerKind": 1
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
                // CompletionList format
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