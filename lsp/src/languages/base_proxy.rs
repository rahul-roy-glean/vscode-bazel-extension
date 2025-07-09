use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command, ChildStdin};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, AsyncReadExt, BufReader};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
use anyhow::{Result, bail};
use serde_json::{json, Value};
use crossbeam_channel::{Sender, Receiver};
use std::collections::HashMap;

pub struct LspConnection {
    process: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    request_id: Arc<Mutex<i64>>,
    pending_requests: Arc<Mutex<HashMap<i64, Sender<Result<Value>>>>>,
    reader_handle: Option<tokio::task::JoinHandle<()>>,
}

impl LspConnection {
    pub async fn new(command: &str, args: &[&str], init_options: Option<Value>) -> Result<Self> {
        let mut process = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = process.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;
        let stdout = process.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;
        
        let stdin = Arc::new(Mutex::new(stdin));
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        
        let mut connection = Self {
            process,
            stdin: stdin.clone(),
            request_id: Arc::new(Mutex::new(1)),
            pending_requests: pending_requests.clone(),
            reader_handle: None,
        };

        // Start reader task
        let reader = BufReader::new(stdout);
        let reader_handle = tokio::spawn(Self::read_messages(reader, pending_requests));
        connection.reader_handle = Some(reader_handle);

        // Initialize the language server
        connection.initialize(init_options).await?;

        Ok(connection)
    }

    async fn read_messages(
        mut reader: BufReader<tokio::process::ChildStdout>,
        pending_requests: Arc<Mutex<HashMap<i64, Sender<Result<Value>>>>>,
    ) {
        let mut headers = HashMap::new();
        let mut content_length = 0;

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if line == "\r\n" || line == "\n" {
                        // Headers complete, read content
                        if content_length > 0 {
                            let mut content = vec![0u8; content_length];
                            if let Err(e) = reader.read_exact(&mut content).await {
                                tracing::error!("Failed to read LSP message content: {}", e);
                                break;
                            }

                            if let Ok(msg) = serde_json::from_slice::<Value>(&content) {
                                Self::handle_message(msg, &pending_requests).await;
                            }
                        }
                        headers.clear();
                        content_length = 0;
                    } else if let Some((key, value)) = line.trim().split_once(": ") {
                        headers.insert(key.to_string(), value.to_string());
                        if key == "Content-Length" {
                            content_length = value.parse().unwrap_or(0);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to read from LSP: {}", e);
                    break;
                }
            }
        }
    }

    async fn handle_message(
        msg: Value,
        pending_requests: &Arc<Mutex<HashMap<i64, Sender<Result<Value>>>>>,
    ) {
        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
            // This is a response
            let mut pending = pending_requests.lock().await;
            if let Some(sender) = pending.remove(&id) {
                if msg.get("error").is_some() {
                    let _ = sender.send(Err(anyhow::anyhow!("LSP error: {:?}", msg["error"])));
                } else if let Some(result) = msg.get("result") {
                    let _ = sender.send(Ok(result.clone()));
                }
            }
        } else if msg.get("method").is_some() {
            // This is a notification or request from server
            tracing::debug!("Received notification from LSP: {:?}", msg["method"]);
        }
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = {
            let mut request_id = self.request_id.lock().await;
            let id = *request_id;
            *request_id += 1;
            id
        };

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let (tx, rx) = crossbeam_channel::bounded(1);
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        self.send_message(request).await?;

        // Wait for response
        match rx.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok(result) => result,
            Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                bail!("LSP request timeout")
            }
        }
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.send_message(notification).await
    }

    async fn send_message(&self, msg: Value) -> Result<()> {
        let content = serde_json::to_string(&msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());
        
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await?;
        stdin.write_all(content.as_bytes()).await?;
        stdin.flush().await?;

        Ok(())
    }

    async fn initialize(&mut self, init_options: Option<Value>) -> Result<()> {
        let params = json!({
            "processId": std::process::id(),
            "clientInfo": {
                "name": "bazel-lsp",
                "version": "0.1.0"
            },
            "capabilities": {
                "textDocument": {
                    "synchronization": {
                        "dynamicRegistration": true,
                        "willSave": true,
                        "willSaveWaitUntil": true,
                        "didSave": true
                    },
                    "completion": {
                        "dynamicRegistration": true,
                        "completionItem": {
                            "snippetSupport": true,
                            "commitCharactersSupport": true,
                            "documentationFormat": ["markdown", "plaintext"]
                        }
                    },
                    "hover": {
                        "dynamicRegistration": true,
                        "contentFormat": ["markdown", "plaintext"]
                    },
                    "definition": {
                        "dynamicRegistration": true,
                        "linkSupport": true
                    }
                }
            },
            "initializationOptions": init_options,
            "workspaceFolders": null
        });

        let _result = self.request("initialize", params).await?;
        self.notify("initialized", json!({})).await?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.request("shutdown", json!({})).await?;
        self.notify("exit", json!({})).await?;
        
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
        
        self.process.kill().await?;
        Ok(())
    }
} 