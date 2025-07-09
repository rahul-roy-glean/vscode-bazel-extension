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

pub struct PythonProxy {
    workspace_root: PathBuf,
    build_graph: Arc<RwLock<BuildGraph>>,
    connection: Arc<Mutex<Option<LspConnection>>>,
}

impl PythonProxy {
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
            // Try to find Python language server (prefer pylsp, fallback to pyright)
            let (server_path, args) = self.find_python_server()?;

            // Configure for Bazel
            let init_options = json!({
                "pylsp": {
                    "plugins": {
                        "pycodestyle": { "enabled": true },
                        "pyflakes": { "enabled": true },
                        "pylint": { "enabled": false },
                        "yapf": { "enabled": true },
                        "rope_completion": { "enabled": true }
                    }
                }
            });

            let lsp_conn = LspConnection::new(
                server_path.to_str().unwrap(),
                &args,
                Some(init_options),
            ).await?;

            // Configure Python environment for Bazel
            self.configure_python(&lsp_conn).await?;

            *conn = Some(lsp_conn);
        }
        Ok(())
    }

    fn find_python_server(&self) -> Result<(PathBuf, Vec<&'static str>)> {
        // Try pylsp first
        if let Ok(pylsp) = which::which("pylsp") {
            return Ok((pylsp, vec![]));
        }

        // Try pyright
        if let Ok(pyright) = which::which("pyright-langserver") {
            return Ok((pyright, vec!["--stdio"]));
        }

        // Try local installations
        let local_candidates = vec![
            (self.workspace_root.join(".venv/bin/pylsp"), vec![]),
            (self.workspace_root.join("venv/bin/pylsp"), vec![]),
            (PathBuf::from("/usr/local/bin/pylsp"), vec![]),
            (PathBuf::from("/usr/bin/pylsp"), vec![]),
        ];

        for (path, args) in local_candidates {
            if path.exists() {
                return Ok((path, args));
            }
        }

        anyhow::bail!("Python language server not found. Install with: pip install python-lsp-server")
    }

    async fn configure_python(&self, conn: &LspConnection) -> Result<()> {
        // Create pyrightconfig.json for better Bazel support
        let pyright_config_path = self.workspace_root.join("pyrightconfig.json");
        if !pyright_config_path.exists() {
            let config = json!({
                "include": [
                    "**/*.py"
                ],
                "exclude": [
                    "**/node_modules",
                    "**/__pycache__",
                    "bazel-*"
                ],
                "extraPaths": [
                    ".",
                    ".bazel/bin",
                    ".bazel/out"
                ],
                "pythonVersion": "3.10.14",
                "typeCheckingMode": "basic"
            });

            let content = serde_json::to_string_pretty(&config)?;
            tokio::fs::write(&pyright_config_path, content).await?;
        }

        // Notify about configuration
        conn.notify("workspace/didChangeConfiguration", json!({
            "settings": {
                "python": {
                    "analysis": {
                        "extraPaths": [
                            self.workspace_root.to_str().unwrap(),
                            self.workspace_root.join(".bazel/bin").to_str().unwrap(),
                            self.workspace_root.join(".bazel/out").to_str().unwrap()
                        ]
                    }
                }
            }
        })).await?;

        Ok(())
    }

    async fn resolve_bazel_import(&self, import_path: &str) -> Option<PathBuf> {
        // Handle Bazel Python imports
        let parts: Vec<&str> = import_path.split('.').collect();
        
        // Check workspace root
        let mut path = self.workspace_root.clone();
        for part in &parts {
            path = path.join(part);
        }
        
        // Try with .py extension
        let py_file = path.with_extension("py");
        if py_file.exists() {
            return Some(py_file);
        }
        
        // Try as package
        let init_file = path.join("__init__.py");
        if init_file.exists() {
            return Some(init_file);
        }

        // Check bazel-bin for generated files
        let bazel_bin = self.workspace_root.join(".bazel/bin");
        if bazel_bin.exists() {
            let mut path = bazel_bin;
            for part in &parts {
                path = path.join(part);
            }
            
            let py_file = path.with_extension("py");
            if py_file.exists() {
                return Some(py_file);
            }
            
            let init_file = path.join("__init__.py");
            if init_file.exists() {
                return Some(init_file);
            }
        }

        None
    }
}

#[async_trait]
impl LanguageServerProxy for PythonProxy {
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