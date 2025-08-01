use pest::Parser;
use pest_derive::Parser;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use dashmap::DashMap;
use tower_lsp::lsp_types::*;
use std::collections::HashMap;
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};

#[derive(Parser)]
#[grammar = "bazel/build.pest"]
pub struct BuildParser;

#[derive(Debug, Clone)]
pub struct BazelTarget {
    pub label: String,
    pub kind: String,
    pub package: String,
    pub srcs: Vec<String>,
    pub deps: Vec<String>,
    pub location: Location,
    pub attributes: HashMap<String, Value>,
}

// Custom Serialize/Deserialize to handle Location
impl Serialize for BazelTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("BazelTarget", 5)?;
        state.serialize_field("label", &self.label)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("package", &self.package)?;
        state.serialize_field("srcs", &self.srcs)?;
        state.serialize_field("deps", &self.deps)?;
        state.end()
    }
}

impl BazelTarget {
    pub fn is_test(&self) -> bool {
        self.kind.ends_with("_test")
    }
}

#[derive(Debug, Clone)]
struct Value {
    kind: ValueKind,
}

#[derive(Debug, Clone)]
enum ValueKind {
    String(String),
    List(Vec<Value>),
    Number(f64),
    Boolean(bool),
}

pub struct BuildGraph {
    targets: DashMap<String, BazelTarget>,
    file_to_targets: DashMap<PathBuf, Vec<String>>,
    workspace_root: Option<PathBuf>,
    // Track reverse dependencies: target -> list of targets that depend on it
    reverse_deps: DashMap<String, Vec<String>>,
}

impl BuildGraph {
    pub fn new() -> Self {
        Self {
            targets: DashMap::new(),
            file_to_targets: DashMap::new(),
            workspace_root: None,
            reverse_deps: DashMap::new(),
        }
    }

    pub async fn scan_workspace(&mut self, root: &Path) -> Result<()> {
        self.workspace_root = Some(root.to_path_buf());
        
        let build_files: Vec<_> = WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                // Skip Bazel output directories (both default bazel-* and custom .bazel/)
                if path.components().any(|c| {
                    if let Some(name) = c.as_os_str().to_str() {
                        name.starts_with("bazel-") || name == ".bazel"
                    } else {
                        false
                    }
                }) {
                    return false;
                }
                
                let name = e.file_name().to_string_lossy();
                name == "BUILD" || name == "BUILD.bazel"
            })
            .map(|e| e.path().to_owned())
            .collect();

        tracing::info!("Found {} BUILD files to parse", build_files.len());

        // Parse BUILD files in parallel using Rayon
        let results: Vec<_> = build_files
            .par_iter()
            .map(|path| self.parse_build_file(path))
            .collect();

        // Process results
        for result in results {
            if let Err(e) = result {
                tracing::warn!("Failed to parse BUILD file: {}", e);
            }
        }

        tracing::info!("Finished scanning workspace, found {} targets", self.targets.len());

        Ok(())
    }

    pub async fn update_build_file(&mut self, path: &Path) -> Result<()> {
        self.parse_build_file(path)
    }

    fn parse_build_file(&self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read BUILD file: {:?}", path))?;

        let pairs = BuildParser::parse(Rule::file, &content)
            .with_context(|| format!("Failed to parse BUILD file: {:?}", path))?;

        let package_path = path.parent()
            .and_then(|p| p.strip_prefix(self.workspace_root.as_ref()?).ok())
            .unwrap_or_else(|| Path::new(""));

        for pair in pairs {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::rule => {
                        if let Some(target) = self.parse_rule(inner, path, package_path)? {
                            let label = target.label.clone();
                            
                            // Update file mappings
                            for src in &target.srcs {
                                let src_path = path.parent().unwrap().join(src);
                                self.file_to_targets
                                    .entry(src_path)
                                    .or_insert_with(Vec::new)
                                    .push(label.clone());
                            }

                            // Update reverse dependencies
                            for dep in &target.deps {
                                self.reverse_deps
                                    .entry(dep.clone())
                                    .or_insert_with(Vec::new)
                                    .push(label.clone());
                            }

                            self.targets.insert(label, target);
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn parse_rule(&self, pair: pest::iterators::Pair<Rule>, path: &Path, package_path: &Path) -> Result<Option<BazelTarget>> {
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap().as_str();
        
        // Skip non-build rules
        if !["cc_library", "cc_binary", "cc_test", "go_library", "go_binary", "go_test", 
             "py_library", "py_binary", "py_test", "java_library", "java_binary", "java_test"]
            .contains(&name) {
            return Ok(None);
        }

        let mut attributes = HashMap::new();
        let mut target_name = String::new();
        let mut srcs = Vec::new();
        let mut deps = Vec::new();

        // Parse arguments
        if let Some(args) = inner.next() {
            for arg in args.into_inner() {
                let mut arg_inner = arg.into_inner();
                let attr_name = arg_inner.next().unwrap().as_str();
                let attr_value = arg_inner.next().unwrap();

                match attr_name {
                    "name" => {
                        target_name = self.extract_string_value(attr_value)?;
                    }
                    "srcs" => {
                        srcs = self.extract_string_list(attr_value)?;
                    }
                    "deps" => {
                        deps = self.extract_string_list(attr_value)?;
                    }
                    _ => {
                        // Store other attributes
                    }
                }
            }
        }

        if target_name.is_empty() {
            return Ok(None);
        }

        let label = if package_path == Path::new("") {
            format!("//:{}", target_name)
        } else {
            format!("//{}:{}", package_path.display(), target_name)
        };

        let location = Location {
            uri: Url::from_file_path(path).unwrap(),
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        };

        let package = package_path.to_string_lossy().to_string();

        Ok(Some(BazelTarget {
            label,
            kind: name.to_string(),
            package,
            srcs,
            deps,
            location,
            attributes,
        }))
    }

    fn extract_string_value(&self, pair: pest::iterators::Pair<Rule>) -> Result<String> {
        match pair.as_rule() {
            Rule::string => {
                let content = pair.as_str();
                Ok(content[1..content.len()-1].to_string())
            }
            _ => Ok(String::new())
        }
    }

    fn extract_string_list(&self, pair: pest::iterators::Pair<Rule>) -> Result<Vec<String>> {
        match pair.as_rule() {
            Rule::list => {
                let mut values = Vec::new();
                for item in pair.into_inner() {
                    if let Ok(s) = self.extract_string_value(item) {
                        values.push(s);
                    }
                }
                Ok(values)
            }
            _ => Ok(Vec::new())
        }
    }

    pub fn get_target_for_file(&self, file: &Url) -> Option<BazelTarget> {
        let path = file.to_file_path().ok()?;
        let targets = self.file_to_targets.get(&path)?;
        targets.first().and_then(|label| {
            self.targets.get(label).map(|t| t.clone())
        })
    }

    pub fn get_code_lenses(&self, uri: &Url) -> Result<Vec<CodeLens>> {
        let path = uri.to_file_path()
            .map_err(|_| anyhow::anyhow!("Invalid URI"))?;
        
        let mut lenses = Vec::new();
        
        // Find all targets in this BUILD file
        for target in self.targets.iter() {
            if target.location.uri == *uri {
                let range = Range::new(Position::new(0, 0), Position::new(0, 0));
                
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title: format!("▶️ Build {}", target.label),
                        command: "bazel.build".to_string(),
                        arguments: Some(vec![serde_json::to_value(&target.label)?]),
                    }),
                    data: None,
                });

                if target.is_test() {
                    lenses.push(CodeLens {
                        range,
                        command: Some(Command {
                            title: format!("🧪 Test {}", target.label),
                            command: "bazel.test".to_string(),
                            arguments: Some(vec![serde_json::to_value(&target.label)?]),
                        }),
                        data: None,
                    });
                }
            }
        }

        Ok(lenses)
    }

    pub fn get_target(&self, label: &str) -> Option<BazelTarget> {
        self.targets.get(label).map(|t| t.clone())
    }

    pub fn get_all_targets(&self) -> Vec<BazelTarget> {
        self.targets.iter().map(|entry| entry.value().clone()).collect()
    }

    pub fn get_targets_in_file(&self, uri: &Url) -> Vec<BazelTarget> {
        self.targets
            .iter()
            .filter(|entry| entry.value().location.uri == *uri)
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub async fn refresh(&mut self) -> Result<()> {
        if let Some(workspace_root) = self.workspace_root.clone() {
            self.scan_workspace(&workspace_root).await
        } else {
            Err(anyhow::anyhow!("Workspace root not set"))
        }
    }

    pub fn find_references(&self, target_label: &str) -> Vec<Location> {
        let mut references = Vec::new();
        
        // Find all targets that depend on this target
        if let Some(dependents) = self.reverse_deps.get(target_label) {
            for dependent_label in dependents.value() {
                if let Some(dependent) = self.targets.get(dependent_label) {
                    references.push(dependent.location.clone());
                }
            }
        }
        
        // Also find references in srcs attributes
        for target in self.targets.iter() {
            // Check if this target is referenced in srcs
            if target.srcs.iter().any(|src| src == target_label) {
                references.push(target.location.clone());
            }
        }
        
        references
    }

    pub fn get_reverse_dependencies(&self, target_label: &str) -> Vec<String> {
        self.reverse_deps
            .get(target_label)
            .map(|deps| deps.clone())
            .unwrap_or_default()
    }

    pub fn get_target_at_position(&self, uri: &Url, position: Position) -> Option<String> {
        // Get all targets in this file
        let targets = self.get_targets_in_file(uri);
        
        // For now, we'll do a simple implementation:
        // Try to read the line at the position and extract a target label
        if let Ok(path) = uri.to_file_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let lines: Vec<&str> = content.lines().collect();
                if let Some(line) = lines.get(position.line as usize) {
                    // Look for Bazel target patterns like //foo:bar or :bar
                    let target_pattern = regex::Regex::new(r#"["']?(//[^"'\s]+|:[^"'\s]+)["']?"#).ok()?;
                    
                    // Find all matches in the line
                    for capture in target_pattern.captures_iter(line) {
                        if let Some(match_) = capture.get(1) {
                            let start_col = match_.start() as u32;
                            let end_col = match_.end() as u32;
                            
                            // Check if position is within this match
                            if position.character >= start_col && position.character <= end_col {
                                let label = match_.as_str();
                                
                                // Handle relative labels (:foo)
                                if label.starts_with(':') {
                                    // Find the package from any target in this file
                                    if let Some(target) = targets.first() {
                                        let package = &target.package;
                                        if package.is_empty() {
                                            return Some(format!("//{}", label));
                                        } else {
                                            return Some(format!("//{}{}", package, label));
                                        }
                                    }
                                } else {
                                    return Some(label.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback: return the first target in the file
        targets.first().map(|t| t.label.clone())
    }
} 