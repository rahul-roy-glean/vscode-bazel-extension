// Build Event Protocol parser
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use anyhow::{Result, Context};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildEvent {
    pub id: BuildEventId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<BuildEventId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<BuildEventPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildEventId {
    #[serde(flatten)]
    pub kind: BuildEventIdKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildEventIdKind {
    #[serde(rename_all = "camelCase")]
    Started { started: Started },
    #[serde(rename_all = "camelCase")]
    Progress { progress: Progress },
    #[serde(rename_all = "camelCase")]
    TargetConfigured { target_configured: TargetConfigured },
    #[serde(rename_all = "camelCase")]
    TargetCompleted { target_completed: TargetCompleted },
    #[serde(rename_all = "camelCase")]
    TestResult { test_result: TestResult },
    #[serde(rename_all = "camelCase")]
    BuildFinished { build_finished: BuildFinished },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Started {
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub opaque_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfigured {
    pub label: String,
    pub aspect: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetCompleted {
    pub label: String,
    pub aspect: Option<String>,
    pub configuration: Option<Configuration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub label: String,
    pub run: i32,
    pub shard: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildFinished {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildEventPayload {
    #[serde(rename_all = "camelCase")]
    Started {
        started: StartedPayload,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        progress: ProgressPayload,
    },
    #[serde(rename_all = "camelCase")]
    TargetConfigured {
        target_configured: TargetConfiguredPayload,
    },
    #[serde(rename_all = "camelCase")]
    TargetCompleted {
        target_completed: TargetCompletedPayload,
    },
    #[serde(rename_all = "camelCase")]
    TestResult {
        test_result: TestResultPayload,
    },
    #[serde(rename_all = "camelCase")]
    BuildFinished {
        finished: BuildFinishedPayload,
    },
    #[serde(rename_all = "camelCase")]
    BuildMetrics {
        build_metrics: BuildMetricsPayload,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedPayload {
    pub uuid: String,
    pub build_tool_version: String,
    pub options_description: Option<String>,
    pub command: String,
    pub working_directory: String,
    pub workspace_directory: String,
    pub server_pid: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub stderr: Option<String>,
    pub stdout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetConfiguredPayload {
    pub target_kind: String,
    pub test_size: Option<String>,
    pub tag: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetCompletedPayload {
    pub success: bool,
    pub output_group: Vec<OutputGroup>,
    pub target_kind: String,
    pub test_size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputGroup {
    pub name: String,
    pub file_sets: Vec<FileSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSet {
    pub files: Vec<File>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub name: String,
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResultPayload {
    pub status: String,
    pub cached_locally: bool,
    pub test_attempt_duration_millis: Option<i64>,
    pub test_logs: Vec<File>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildFinishedPayload {
    pub overall_success: bool,
    pub exit_code: ExitCode,
    pub finish_time_millis: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitCode {
    pub name: String,
    pub code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildMetricsPayload {
    pub action_summary: ActionSummary,
    pub memory_metrics: MemoryMetrics,
    pub target_metrics: TargetMetrics,
    pub timing_metrics: TimingMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSummary {
    pub actions_executed: i64,
    pub actions_created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMetrics {
    pub used_heap_size_post_build: i64,
    pub peak_post_gc_heap_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetMetrics {
    pub targets_configured: i32,
    pub targets_loaded: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingMetrics {
    pub wall_time_millis: i64,
    pub cpu_time_millis: i64,
    pub actions_execution_start_millis: i64,
}

pub struct BuildEventProtocolParser {
    events: HashMap<String, BuildEvent>,
}

impl BuildEventProtocolParser {
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
        }
    }
    
    pub fn parse_event_line(&mut self, line: &str) -> Result<Option<BuildEvent>> {
        let event: BuildEvent = serde_json::from_str(line)
            .context("Failed to parse BEP JSON")?;
        
        // Store event by ID for correlation
        let event_id = self.get_event_id_string(&event.id);
        self.events.insert(event_id, event.clone());
        
        Ok(Some(event))
    }
    
    pub fn parse_event(&self, json: &str) -> Result<BuildEvent> {
        serde_json::from_str(json).context("Failed to parse BEP JSON")
    }
    
    fn get_event_id_string(&self, id: &BuildEventId) -> String {
        match &id.kind {
            BuildEventIdKind::Started { started } => format!("started:{}", started.uuid),
            BuildEventIdKind::Progress { progress } => format!("progress:{}", progress.opaque_count),
            BuildEventIdKind::TargetConfigured { target_configured } => {
                format!("configured:{}", target_configured.label)
            }
            BuildEventIdKind::TargetCompleted { target_completed } => {
                format!("completed:{}", target_completed.label)
            }
            BuildEventIdKind::TestResult { test_result } => {
                format!("test:{}:{}:{}", test_result.label, test_result.run, test_result.shard)
            }
            BuildEventIdKind::BuildFinished { .. } => "finished".to_string(),
        }
    }
    
    pub fn get_build_status(&self) -> Option<bool> {
        self.events.values()
            .find_map(|event| {
                if let Some(BuildEventPayload::BuildFinished { finished }) = &event.payload {
                    Some(finished.overall_success)
                } else {
                    None
                }
            })
    }
    
    pub fn get_test_results(&self) -> Vec<(String, bool)> {
        self.events.values()
            .filter_map(|event| {
                if let Some(BuildEventPayload::TestResult { test_result }) = &event.payload {
                    if let BuildEventIdKind::TestResult { test_result: id } = &event.id.kind {
                        Some((id.label.clone(), test_result.status == "PASSED"))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
    
    pub fn get_output_files(&self) -> Vec<(String, Vec<String>)> {
        self.events.values()
            .filter_map(|event| {
                if let Some(BuildEventPayload::TargetCompleted { target_completed }) = &event.payload {
                    if let BuildEventIdKind::TargetCompleted { target_completed: id } = &event.id.kind {
                        let files: Vec<String> = target_completed.output_group
                            .iter()
                            .flat_map(|group| &group.file_sets)
                            .flat_map(|set| &set.files)
                            .map(|file| file.uri.clone())
                            .collect();
                        
                        if !files.is_empty() {
                            Some((id.label.clone(), files))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
} 