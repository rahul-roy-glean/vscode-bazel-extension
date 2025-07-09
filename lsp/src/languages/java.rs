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

pub struct JavaProxy {
    workspace_root: PathBuf,
    build_graph: Arc<RwLock<BuildGraph>>,
    connection: Arc<Mutex<Option<LspConnection>>>,
}

impl JavaProxy {
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
            // Find Java language server (jdtls)
            let jdtls_path = self.find_jdtls()
                .context("Eclipse JDT Language Server not found")?;

            // Set up workspace for jdtls
            let workspace_data = self.workspace_root.join(".jdtls-workspace");
            tokio::fs::create_dir_all(&workspace_data).await?;

            // Configure for Bazel
            let init_options = json!({
                "bundles": [],
                "workspaceFolders": [
                    format!("file://{}", self.workspace_root.display())
                ],
                "settings": {
                    "java": {
                        "home": self.find_java_home(),
                        "import": {
                            "gradle": { "enabled": false },
                            "maven": { "enabled": false },
                            "bazel": { "enabled": true }
                        },
                        "configuration": {
                            "runtimes": []
                        },
                        "project": {
                            "referencedLibraries": [
                                ".bazel/bin/**/*.jar",
                                ".bazel/out/**/*.jar"
                            ]
                        }
                    }
                }
            });

            let launcher_path = self.find_jdtls_launcher(&jdtls_path)?;
            let config_path = self.find_jdtls_config(&jdtls_path)?;
            
            let args = vec![
                "-Declipse.application=org.eclipse.jdt.ls.core.id1",
                "-Dosgi.bundles.defaultStartLevel=4",
                "-Declipse.product=org.eclipse.jdt.ls.core.product",
                "-Dlog.level=ALL",
                "-noverify",
                "-Xmx1G",
                "--add-modules=ALL-SYSTEM",
                "--add-opens", "java.base/java.util=ALL-UNNAMED",
                "--add-opens", "java.base/java.lang=ALL-UNNAMED",
                "-jar", &launcher_path,
                "-configuration", &config_path,
                "-data", workspace_data.to_str().unwrap(),
            ];

            let lsp_conn = LspConnection::new(
                "java",
                &args.iter().map(|s| *s).collect::<Vec<_>>(),
                Some(init_options),
            ).await?;

            *conn = Some(lsp_conn);
        }
        Ok(())
    }

    fn find_jdtls(&self) -> Result<PathBuf> {
        // Try common locations
        let candidates = vec![
            // VSCode extension location
            dirs::home_dir().map(|h| h.join(".vscode/extensions").join("redhat.java-*/server")),
            // Manual installation
            Some(PathBuf::from("/opt/jdtls")),
            Some(PathBuf::from("/usr/local/opt/jdtls")),
            // Homebrew on macOS
            Some(PathBuf::from("/usr/local/Cellar/jdtls/*/libexec")),
        ];

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                // Look for versioned directories
                if let Ok(entries) = std::fs::read_dir(&candidate) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.to_string_lossy().contains("jdt") {
                            return Ok(path);
                        }
                    }
                }
                return Ok(candidate);
            }
        }

        anyhow::bail!("Eclipse JDT Language Server not found. Please install it manually.")
    }

    fn find_jdtls_launcher(&self, jdtls_path: &PathBuf) -> Result<String> {
        let plugins_dir = jdtls_path.join("plugins");
        if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("org.eclipse.equinox.launcher_") && name_str.ends_with(".jar") {
                    return Ok(entry.path().to_string_lossy().to_string());
                }
            }
        }
        anyhow::bail!("JDTLS launcher JAR not found")
    }

    fn find_jdtls_config(&self, jdtls_path: &PathBuf) -> Result<String> {
        let config_dir = jdtls_path.join("config_linux");
        if config_dir.exists() {
            return Ok(config_dir.to_string_lossy().to_string());
        }
        
        let config_dir = jdtls_path.join("config_mac");
        if config_dir.exists() {
            return Ok(config_dir.to_string_lossy().to_string());
        }
        
        let config_dir = jdtls_path.join("config_win");
        if config_dir.exists() {
            return Ok(config_dir.to_string_lossy().to_string());
        }

        anyhow::bail!("JDTLS config directory not found")
    }

    fn find_java_home(&self) -> Option<String> {
        // Try JAVA_HOME first
        if let Ok(java_home) = std::env::var("JAVA_HOME") {
            return Some(java_home);
        }

        // Try common locations
        let candidates = vec![
            "/Library/Java/JavaVirtualMachines/openjdk-17.jdk/Contents/Home",
            "/usr/lib/jvm/java-11-openjdk-amd64",
            "/Library/Java/JavaVirtualMachines/adoptopenjdk-11.jdk/Contents/Home",
            "/System/Library/Frameworks/JavaVM.framework/Versions/Current",
        ];

        for candidate in candidates {
            if PathBuf::from(candidate).exists() {
                return Some(candidate.to_string());
            }
        }

        None
    }

    async fn resolve_bazel_target(&self, class_name: &str) -> Option<PathBuf> {
        // Convert Java class name to file path
        let path = class_name.replace('.', "/") + ".java";
        
        // Check in workspace
        let src_paths = vec![
            self.workspace_root.join("java/com/askscio").join(&path),
            self.workspace_root.join("javatests/com/askscio").join(&path),
            self.workspace_root.join(&path),
        ];

        for src_path in src_paths {
            if src_path.exists() {
                return Some(src_path);
            }
        }

        // Check in bazel-bin for generated files
        let bazel_bin = self.workspace_root.join(".bazel/bin");
        if bazel_bin.exists() {
            let generated = bazel_bin.join(&path);
            if generated.exists() {
                return Some(generated);
            }
        }

        None
    }
}

#[async_trait]
impl LanguageServerProxy for JavaProxy {
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