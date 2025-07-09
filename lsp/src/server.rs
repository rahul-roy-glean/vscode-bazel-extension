use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use tower_lsp::{Client, LanguageServer};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use std::path::PathBuf;
use serde_json::Value;
use crate::bazel::{BazelClient, BuildGraph};
use crate::languages::LanguageCoordinator;

pub struct BazelLanguageServer {
    client: Client,
    build_graph: Arc<RwLock<BuildGraph>>,
    bazel_client: Arc<BazelClient>,
    language_coordinator: Arc<LanguageCoordinator>,
    document_cache: Arc<DashMap<Url, String>>,
    workspace_root: Arc<RwLock<Option<PathBuf>>>,
}

impl BazelLanguageServer {
    pub fn new(client: Client) -> Self {
        let build_graph = Arc::new(RwLock::new(BuildGraph::new()));
        let bazel_client = Arc::new(BazelClient::new());
        let language_coordinator = Arc::new(LanguageCoordinator::new(build_graph.clone()));
        
        Self {
            client,
            build_graph,
            bazel_client,
            language_coordinator,
            document_cache: Arc::new(DashMap::new()),
            workspace_root: Arc::new(RwLock::new(None)),
        }
    }
    
    async fn extract_bazel_target(&self, uri: &Url, position: Position) -> Option<String> {
        let content = self.document_cache.get(uri)?;
        let lines: Vec<&str> = content.split('\n').collect();
        let line = lines.get(position.line as usize)?;
        
        // Simple regex for Bazel target references like //path/to:target
        let re = regex::Regex::new(r"//[a-zA-Z0-9_/:-]+").ok()?;
        
        for cap in re.captures_iter(line) {
            if let Some(target) = cap.get(0) {
                let target_str = target.as_str();
                let start = target.start() as u32;
                let end = target.end() as u32;
                
                if position.character >= start && position.character <= end {
                    return Some(target_str.to_string());
                }
            }
        }
        
        None
    }
    
    async fn resolve_bazel_target(&self, target_ref: &str) -> Option<Location> {
        let workspace_root = self.workspace_root.read().await;
        let root = workspace_root.as_ref()?;
        
        // Parse target reference (e.g., //path/to:target)
        let parts: Vec<&str> = target_ref.trim_start_matches("//").split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        
        let path = parts[0];
        let _target_name = parts[1];
        
        // Try BUILD or BUILD.bazel
        for build_file in ["BUILD", "BUILD.bazel"] {
            let build_path = root.join(path).join(build_file);
            if build_path.exists() {
                return Some(Location {
                    uri: Url::from_file_path(build_path).ok()?,
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                });
            }
        }
        
        None
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for BazelLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let workspace_root = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        // Store workspace root
        {
            let mut root = self.workspace_root.write().await;
            *root = Some(workspace_root.clone());
        }

        // Initialize bazel client with workspace root
        self.bazel_client.set_workspace_root(workspace_root.clone()).await;

        // Initialize language coordinator
        if let Err(e) = self.language_coordinator.initialize(workspace_root.clone()).await {
            tracing::error!("Failed to initialize language coordinator: {}", e);
        }

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
                // workspace_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Bazel Language Server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        
        self.document_cache.insert(uri.clone(), content);
        
        // If it's a BUILD file, update the build graph
        if uri.path().ends_with("BUILD") || uri.path().ends_with("BUILD.bazel") {
            if let Ok(path) = uri.to_file_path() {
                let build_graph = self.build_graph.clone();
                tokio::spawn(async move {
                    let mut graph = build_graph.write().await;
                    if let Err(e) = graph.update_build_file(&path).await {
                        tracing::warn!("Failed to update BUILD file: {}", e);
                    }
                });
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        
        if let Some(mut content) = self.document_cache.get_mut(&uri) {
            for change in params.content_changes {
                if let Some(range) = change.range {
                    // Apply incremental change
                    let lines: Vec<String> = content.split('\n').map(String::from).collect();
                    let mut new_lines = lines.clone();
                    
                    // Simple implementation - replace range with new text
                    // In production, this would need proper text manipulation
                    *content = change.text;
                } else {
                    // Full document sync
                    *content = change.text;
                }
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        
        // Update build graph if it's a BUILD file
        if uri.path().ends_with("BUILD") || uri.path().ends_with("BUILD.bazel") {
            if let Ok(path) = uri.to_file_path() {
                let build_graph = self.build_graph.clone();
                tokio::spawn(async move {
                    let mut graph = build_graph.write().await;
                    if let Err(e) = graph.update_build_file(&path).await {
                        tracing::warn!("Failed to update BUILD file: {}", e);
                    }
                });
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.document_cache.remove(&params.text_document.uri);
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
        match self.language_coordinator.goto_definition(uri, position).await {
            Ok(response) => Ok(response),
            Err(e) => {
                tracing::error!("goto_definition error: {}", e);
                Ok(None)
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // Check if we're in a BUILD file
        if uri.path().ends_with("BUILD") || uri.path().ends_with("BUILD.bazel") {
            // Provide Bazel-specific completions
            let items = vec![
                CompletionItem {
                    label: "cc_library".to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("C++ library rule".to_string()),
                    ..Default::default()
                },
                CompletionItem {
                    label: "cc_binary".to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("C++ binary rule".to_string()),
                    ..Default::default()
                },
                CompletionItem {
                    label: "cc_test".to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("C++ test rule".to_string()),
                    ..Default::default()
                },
                CompletionItem {
                    label: "go_library".to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Go library rule".to_string()),
                    ..Default::default()
                },
                CompletionItem {
                    label: "go_binary".to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Go binary rule".to_string()),
                    ..Default::default()
                },
                CompletionItem {
                    label: "go_test".to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Go test rule".to_string()),
                    ..Default::default()
                },
            ];
            
            return Ok(Some(CompletionResponse::Array(items)));
        }

        // Delegate to language-specific handler
        match self.language_coordinator.completion(uri, position).await {
            Ok(items) => Ok(Some(CompletionResponse::Array(items))),
            Err(e) => {
                tracing::error!("completion error: {}", e);
                Ok(None)
            }
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Check if hovering over a Bazel target
        if let Some(target_ref) = self.extract_bazel_target(&uri, position).await {
            // Query Bazel for target info
            match self.bazel_client.query_target_info(&target_ref).await {
                Ok(info) => {
                    let content = MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!(
                            "**Bazel Target**: `{}`\n\n**Kind**: {}\n\n**Visibility**: {}",
                            target_ref, info.kind, info.visibility
                        ),
                    };
                    
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(content),
                        range: None,
                    }));
                }
                Err(e) => {
                    tracing::warn!("Failed to query target info: {}", e);
                }
            }
        }

        // Delegate to language-specific handler
        match self.language_coordinator.hover(uri, position).await {
            Ok(hover) => Ok(hover),
            Err(e) => {
                tracing::error!("hover error: {}", e);
                Ok(None)
            }
        }
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        
        if uri.path().ends_with("BUILD") || uri.path().ends_with("BUILD.bazel") {
            let build_graph = self.build_graph.read().await;
            match build_graph.get_code_lenses(&uri) {
                Ok(lenses) => Ok(Some(lenses)),
                Err(e) => {
                    tracing::error!("code_lens error: {}", e);
                    Ok(None)
                }
            }
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
                                arguments: Some(vec![serde_json::to_value(&target.label).unwrap()]),
                            }),
                            data: None,
                        },
                        CodeLens {
                            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                            command: Some(Command {
                                title: "🐛 Debug Test".to_string(),
                                command: "bazel.debug".to_string(),
                                arguments: Some(vec![serde_json::to_value(&target.label).unwrap()]),
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

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        
        tracing::info!("References request for {:?} at {:?}", uri, position);
        
        // Check if this is a BUILD file
        let path = uri.path();
        if path.ends_with("BUILD") || path.ends_with("BUILD.bazel") {
            // Handle Bazel target references
            let build_graph = self.build_graph.read().await;
            
            // Find the target at the current position
            if let Some(target_label) = build_graph.get_target_at_position(&uri, position) {
                let references = build_graph.find_references(&target_label);
                
                tracing::info!("Found {} references to target {}", references.len(), target_label);
                
                return Ok(Some(references));
            }
        } else {
            // For source files, delegate to the appropriate language server
            let file_path = match uri.to_file_path() {
                Ok(path) => path,
                Err(_) => return Ok(Some(Vec::new()))
            };
            
            // Determine file type and delegate
            if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
                match extension {
                    "go" => {
                        // In a full implementation, we would delegate to the Go language server
                        tracing::info!("Would delegate Go references request to Go language server");
                    }
                    "py" => {
                        // In a full implementation, we would delegate to the Python language server
                        tracing::info!("Would delegate Python references request to Python language server");
                    }
                    "java" => {
                        // In a full implementation, we would delegate to the Java language server
                        tracing::info!("Would delegate Java references request to Java language server");
                    }
                    "ts" | "js" => {
                        // In a full implementation, we would delegate to the TypeScript language server
                        tracing::info!("Would delegate TypeScript references request to TypeScript language server");
                    }
                    _ => {}
                }
            }
        }
        
        Ok(Some(Vec::new()))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        
        // For BUILD files, return symbols for targets
        if uri.path().ends_with("BUILD") || uri.path().ends_with("BUILD.bazel") {
            let build_graph = self.build_graph.read().await;
            let mut symbols = Vec::new();
            
            for target in build_graph.get_targets_in_file(&uri) {
                let symbol = DocumentSymbol {
                    name: target.label.clone(),
                    detail: Some(target.kind.clone()),
                    kind: SymbolKind::FUNCTION,
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    selection_range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    children: None,
                    tags: None,
                    deprecated: None,
                };
                symbols.push(symbol);
            }
            
            return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
        }
        
        // For other files, we could delegate to language servers but for now return empty
        Ok(None)
    }

    // Commands are now handled client-side, so this is no longer needed
    /*
    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        match params.command.as_str() {
            "bazel.build" => {
                let target = params.arguments.get(0)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
                
                self.bazel_client.build(target).await
                    .map_err(|e| tower_lsp::jsonrpc::Error::internal_error())?;
                Ok(None)
            }
            "bazel.test" => {
                let target = params.arguments.get(0)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
                
                self.bazel_client.test(target).await
                    .map_err(|e| tower_lsp::jsonrpc::Error::internal_error())?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
    */
}

impl BazelLanguageServer {
    pub async fn handle_custom_request(&self, method: &str, params: Value) -> Result<Value> {
        match method {
            "bazel/getTargetForFile" => {
                let uri = params.get("uri")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing uri"))?;
                
                let url = Url::parse(uri).map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid URI: {}", e)))?;
                let build_graph = self.build_graph.read().await;
                
                if let Some(target) = build_graph.get_target_for_file(&url) {
                    Ok(serde_json::json!({ "target": target.label }))
                } else {
                    Ok(serde_json::json!({ "target": null }))
                }
            }
            "bazel/getDependencies" => {
                let target = params.get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
                
                let build_graph = self.build_graph.read().await;
                if let Some(target_info) = build_graph.get_target(target) {
                    Ok(serde_json::json!(target_info.deps))
                } else {
                    Ok(serde_json::json!([]))
                }
            }
            "bazel/getAllTargets" => {
                let build_graph = self.build_graph.read().await;
                let targets = build_graph.get_all_targets();
                serde_json::to_value(targets)
                    .map_err(|e| tower_lsp::jsonrpc::Error::internal_error())
            }
            "bazel/getTargetLocation" => {
                let target = params.get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
                
                let build_graph = self.build_graph.read().await;
                if let Some(target_info) = build_graph.get_target(target) {
                    Ok(serde_json::json!({
                        "uri": target_info.location.uri.to_string(),
                        "range": target_info.location.range
                    }))
                } else {
                    Ok(serde_json::json!(null))
                }
            }
            _ => Err(tower_lsp::jsonrpc::Error::method_not_found()),
        }
    }

    pub async fn handle_custom_notification(&self, method: &str, _params: Value) -> Result<()> {
        match method {
            "bazel/refreshWorkspace" => {
                let build_graph = self.build_graph.clone();
                
                // Refresh in background
                tokio::spawn(async move {
                    let mut graph = build_graph.write().await;
                    if let Err(e) = graph.refresh().await {
                        tracing::error!("Failed to refresh workspace: {}", e);
                    }
                });
                
                // Notify clients that targets have changed
                // For now, just log it. The TypeScript side will need to poll for changes
                self.client
                    .log_message(MessageType::INFO, "Workspace refreshed")
                    .await;
                Ok(())
            }
            _ => Ok(()), // Ignore unknown notifications
        }
    }

    // Custom method handlers for tower-lsp
    pub async fn bazel_get_target_for_file(&self, params: Value) -> Result<Value> {
        let uri = params.get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing uri"))?;
        
        let url = Url::parse(uri).map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid URI: {}", e)))?;
        let build_graph = self.build_graph.read().await;
        
        if let Some(target) = build_graph.get_target_for_file(&url) {
            Ok(serde_json::json!({ "target": target.label }))
        } else {
            Ok(serde_json::json!({ "target": null }))
        }
    }

    pub async fn bazel_get_dependencies(&self, params: Value) -> Result<Value> {
        let target = params.get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
        
        let build_graph = self.build_graph.read().await;
        if let Some(target_info) = build_graph.get_target(target) {
            Ok(serde_json::json!(target_info.deps))
        } else {
            Ok(serde_json::json!([]))
        }
    }

    pub async fn bazel_get_all_targets(&self, _params: Value) -> Result<Value> {
        let build_graph = self.build_graph.read().await;
        let targets = build_graph.get_all_targets();
        serde_json::to_value(targets)
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())
    }

    pub async fn bazel_get_target_location(&self, params: Value) -> Result<Value> {
        let target = params.get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing target"))?;
        
        let build_graph = self.build_graph.read().await;
        if let Some(target_info) = build_graph.get_target(target) {
            Ok(serde_json::json!({
                "uri": target_info.location.uri.to_string(),
                "range": target_info.location.range
            }))
        } else {
            Ok(serde_json::json!(null))
        }
    }

    pub async fn bazel_refresh_workspace(&self, _params: Value) -> Result<Value> {
        let mut build_graph = self.build_graph.write().await;
        build_graph.refresh().await
            .map_err(|e| tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                message: format!("Failed to refresh workspace: {}", e).into(),
                data: None,
            })?;
        
        Ok(serde_json::json!({
            "success": true
        }))
    }

    pub async fn bazel_get_target_dependencies(&self, params: Value) -> Result<Value> {
        let target_label = params.get("targetLabel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::InvalidParams,
                message: "Missing targetLabel parameter".into(),
                data: None,
            })?;
        
        let build_graph = self.build_graph.read().await;
        
        // Get the target
        let target = build_graph.get_target(&target_label);
        
        // Get reverse dependencies
        let reverse_deps = build_graph.get_reverse_dependencies(&target_label);
        
        Ok(serde_json::json!({
            "targetLabel": target_label,
            "dependencies": target.as_ref().map(|t| &t.deps).unwrap_or(&Vec::new()),
            "reverseDependencies": reverse_deps,
            "exists": target.is_some()
        }))
    }

    pub async fn custom_references(&self, params: Value) -> Result<Value> {
        // Parse the ReferenceParams from the incoming JSON
        let reference_params: ReferenceParams = serde_json::from_value(params)
            .map_err(|e| tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::InvalidParams,
                message: format!("Invalid reference parameters: {}", e).into(),
                data: None,
            })?;
        
        // Call the existing references implementation
        let result = self.references(reference_params).await?;
        
        // Convert the result back to JSON
        Ok(serde_json::to_value(result)
            .map_err(|e| tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                message: format!("Failed to serialize result: {}", e).into(),
                data: None,
            })?)
    }
} 