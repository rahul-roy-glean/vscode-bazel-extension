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
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .init();

    tracing::info!("Starting Bazel Language Server");

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    
    let (service, socket) = LspService::build(|client| {
        BazelLanguageServer::new(client)
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
} 