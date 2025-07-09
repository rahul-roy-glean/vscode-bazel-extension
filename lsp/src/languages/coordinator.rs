use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use dashmap::DashMap;
use async_trait::async_trait;
use anyhow::Result;
use crate::bazel::BuildGraph;

pub struct LanguageCoordinator {
    workspace_root: Arc<RwLock<Option<PathBuf>>>,
    build_graph: Arc<RwLock<BuildGraph>>,
    language_servers: DashMap<String, Arc<Box<dyn LanguageServerProxy>>>,
}

#[async_trait]
pub trait LanguageServerProxy: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
    async fn goto_definition(&self, uri: Url, position: Position) -> Result<Option<Location>>;
    async fn completion(&self, uri: Url, position: Position) -> Result<Vec<CompletionItem>>;
    async fn hover(&self, uri: Url, position: Position) -> Result<Option<Hover>>;
}

impl LanguageCoordinator {
    pub fn new(build_graph: Arc<RwLock<BuildGraph>>) -> Self {
        Self {
            workspace_root: Arc::new(RwLock::new(None)),
            build_graph,
            language_servers: DashMap::new(),
        }
    }

    pub async fn initialize(&self, workspace_root: PathBuf) -> Result<()> {
        {
            let mut root = self.workspace_root.write().await;
            *root = Some(workspace_root.clone());
        }

        // Initialize language servers
        self.initialize_language_servers(workspace_root).await?;
        Ok(())
    }

    async fn initialize_language_servers(&self, workspace_root: PathBuf) -> Result<()> {
        // Initialize Go proxy
        let mut go_proxy = Box::new(GoProxy::new(workspace_root.clone(), self.build_graph.clone()));
        if let Err(e) = go_proxy.start().await {
            tracing::warn!("Failed to start Go language server: {}", e);
        } else {
            self.language_servers.insert("go".to_string(), Arc::new(go_proxy));
        }

        // Initialize TypeScript proxy
        let mut ts_proxy = Box::new(TypeScriptProxy::new(workspace_root.clone(), self.build_graph.clone()));
        if let Err(e) = ts_proxy.start().await {
            tracing::warn!("Failed to start TypeScript language server: {}", e);
        } else {
            self.language_servers.insert("typescript".to_string(), Arc::new(ts_proxy));
        }

        // Initialize Python proxy
        let mut py_proxy = Box::new(PythonProxy::new(workspace_root.clone(), self.build_graph.clone()));
        if let Err(e) = py_proxy.start().await {
            tracing::warn!("Failed to start Python language server: {}", e);
        } else {
            self.language_servers.insert("python".to_string(), Arc::new(py_proxy));
        }

        // Initialize Java proxy
        let mut java_proxy = Box::new(JavaProxy::new(workspace_root.clone(), self.build_graph.clone()));
        if let Err(e) = java_proxy.start().await {
            tracing::warn!("Failed to start Java language server: {}", e);
        } else {
            self.language_servers.insert("java".to_string(), Arc::new(java_proxy));
        }

        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        // Note: We can't get mutable access through Arc in a shared reference
        // In a real implementation, we'd need a different approach
        // For now, just clear the servers
        self.language_servers.clear();
        Ok(())
    }

    pub async fn goto_definition(
        &self,
        uri: Url,
        position: Position,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let language = self.get_language_for_uri(&uri);
        
        if let Some(proxy) = self.language_servers.get(&language) {
            if let Some(location) = proxy.goto_definition(uri, position).await? {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
            }
        }

        Ok(None)
    }

    pub async fn completion(
        &self,
        uri: Url,
        position: Position,
    ) -> Result<Vec<CompletionItem>> {
        let language = self.get_language_for_uri(&uri);
        
        if let Some(proxy) = self.language_servers.get(&language) {
            return proxy.completion(uri, position).await;
        }

        Ok(Vec::new())
    }

    pub async fn hover(
        &self,
        uri: Url,
        position: Position,
    ) -> Result<Option<Hover>> {
        let language = self.get_language_for_uri(&uri);
        
        if let Some(proxy) = self.language_servers.get(&language) {
            return proxy.hover(uri, position).await;
        }

        Ok(None)
    }

    fn get_language_for_uri(&self, uri: &Url) -> String {
        let ext = uri.path()
            .split('.')
            .last()
            .unwrap_or("");

        match ext {
            "go" => "go",
            "ts" | "tsx" | "js" | "jsx" => "typescript",
            "py" => "python",
            "java" => "java",
            _ => "unknown",
        }.to_string()
    }
}

// Import language proxy implementations
use super::go::GoProxy;
use super::typescript::TypeScriptProxy;
use super::python::PythonProxy;
use super::java::JavaProxy; 