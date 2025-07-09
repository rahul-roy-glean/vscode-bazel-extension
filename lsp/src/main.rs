mod server;
mod bazel;
mod languages;
mod cache;

use server::BazelLanguageServer;
use std::sync::Arc;
use tower_lsp::{LspService, Server};
use tracing_subscriber;

#[tokio::main]
async fn main() {
    // Initialize logging to stderr (stdout is used for LSP communication)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Bazel Language Server");

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    
    let (service, socket) = LspService::build(|client| {
        BazelLanguageServer::new(client)
    })
    .custom_method("bazel/getTargetForFile", BazelLanguageServer::bazel_get_target_for_file)
    .custom_method("bazel/getDependencies", BazelLanguageServer::bazel_get_dependencies)
    .custom_method("bazel/getAllTargets", BazelLanguageServer::bazel_get_all_targets)
    .custom_method("bazel/getTargetLocation", BazelLanguageServer::bazel_get_target_location)
    .custom_method("bazel/refreshWorkspace", BazelLanguageServer::bazel_refresh_workspace)
    .custom_method("bazel/getTargetDependencies", BazelLanguageServer::bazel_get_target_dependencies)
    .custom_method("textDocument/references", BazelLanguageServer::custom_references)
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
} 