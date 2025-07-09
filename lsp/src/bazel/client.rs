use std::path::PathBuf;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::sync::Arc;
use tokio::sync::Mutex;
use lru::LruCache;
use std::num::NonZeroUsize;
use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub struct BuildResult {
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TargetInfo {
    pub kind: String,
    pub visibility: String,
}

pub struct BazelClient {
    workspace_root: Arc<Mutex<Option<PathBuf>>>,
    bazel_path: PathBuf,
    query_cache: Arc<Mutex<LruCache<String, QueryResult>>>,
}

impl BazelClient {
    pub fn new() -> Self {
        let bazel_path = which::which("bazel").unwrap_or_else(|_| PathBuf::from("bazel"));
        
        Self {
            workspace_root: Arc::new(Mutex::new(None)),
            bazel_path,
            query_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(1000).unwrap()
            ))),
        }
    }
    
    pub async fn set_workspace_root(&self, root: PathBuf) {
        let mut workspace_root = self.workspace_root.lock().await;
        *workspace_root = Some(root);
    }

    pub async fn query(&self, query: &str) -> Result<QueryResult> {
        // Check cache first
        {
            let mut cache = self.query_cache.lock().await;
            if let Some(result) = cache.get(query) {
                return Ok(result.clone());
            }
        }

        let workspace_root = self.workspace_root.lock().await;
        let root = workspace_root.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Workspace root not set"))?;

        let output = Command::new(&self.bazel_path)
            .current_dir(root)
            .args(&[
                "query",
                query,
                "--output=proto",
            ])
            .output()
            .await?;

        if !output.status.success() {
            bail!("Bazel query failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        // Try to parse as protobuf first
        let targets = if let Ok(parser) = super::QueryParser::new().parse_proto_output(&output.stdout) {
            parser.targets.into_iter().map(|t| t.name).collect()
        } else {
            // Fallback to text parsing
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout
                .lines()
                .filter(|line| !line.is_empty())
                .map(|s| s.to_string())
                .collect()
        };

        let result = QueryResult { targets };
        
        // Cache result
        {
            let mut cache = self.query_cache.lock().await;
            cache.put(query.to_string(), result.clone());
        }

        Ok(result)
    }

    pub async fn query_target_info(&self, target: &str) -> Result<TargetInfo> {
        let workspace_root = self.workspace_root.lock().await;
        let root = workspace_root.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Workspace root not set"))?;

        let output = Command::new(&self.bazel_path)
            .current_dir(root)
            .args(&[
                "query",
                &format!("kind('.*', {})", target),
                "--output=label_kind",
            ])
            .output()
            .await?;

        if !output.status.success() {
            bail!("Bazel query failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        
        if let Some(line) = lines.first() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return Ok(TargetInfo {
                    kind: parts[0].to_string(),
                    visibility: "//visibility:public".to_string(), // Default for now
                });
            }
        }

        bail!("Failed to parse target info")
    }

    pub async fn build(&self, target: &str) -> Result<BuildResult> {
        let workspace_root = self.workspace_root.lock().await;
        let root = workspace_root.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Workspace root not set"))?;

        // Create a temporary file for BEP output
        let bep_file = tempfile::NamedTempFile::new()?;
        let bep_path = bep_file.path().to_str().unwrap();

        let mut child = Command::new(&self.bazel_path)
            .current_dir(root)
            .args(&[
                "build", 
                target,
                &format!("--build_event_json_file={}", bep_path),
                "--build_event_publish_all_actions",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let status = child.wait().await?;
        
        // Parse BEP output
        let mut parser = super::BuildEventProtocolParser::new();
        if let Ok(content) = tokio::fs::read_to_string(&bep_path).await {
            for line in content.lines() {
                if let Err(e) = parser.parse_event_line(line) {
                    tracing::warn!("Failed to parse BEP line: {}", e);
                }
            }
        }
        
        // Get overall build status from BEP or fallback to exit code
        let success = parser.get_build_status().unwrap_or(status.success());
        
        Ok(BuildResult { success })
    }

    pub async fn test(&self, target: &str) -> Result<TestResult> {
        let workspace_root = self.workspace_root.lock().await;
        let root = workspace_root.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Workspace root not set"))?;

        // Create a temporary file for BEP output
        let bep_file = tempfile::NamedTempFile::new()?;
        let bep_path = bep_file.path().to_str().unwrap();

        let mut child = Command::new(&self.bazel_path)
            .current_dir(root)
            .args(&[
                "test", 
                target,
                &format!("--build_event_json_file={}", bep_path),
                "--test_output=errors",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let status = child.wait().await?;
        
        // Parse BEP output
        let mut parser = super::BuildEventProtocolParser::new();
        if let Ok(content) = tokio::fs::read_to_string(&bep_path).await {
            for line in content.lines() {
                if let Err(e) = parser.parse_event_line(line) {
                    tracing::warn!("Failed to parse BEP line: {}", e);
                }
            }
        }
        
        // Get test results from BEP
        let test_results = parser.get_test_results();
        let success = if test_results.is_empty() {
            status.success()
        } else {
            test_results.iter().all(|(_, passed)| *passed)
        };
        
        Ok(TestResult { success })
    }

    pub async fn run(&self, target: &str) -> Result<()> {
        let workspace_root = self.workspace_root.lock().await;
        let root = workspace_root.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Workspace root not set"))?;

        let mut child = Command::new(&self.bazel_path)
            .current_dir(root)
            .args(&["run", target])
            .spawn()?;

        child.wait().await?;
        Ok(())
    }
} 