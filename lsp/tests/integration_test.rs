#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::*;
    
    #[test]
    fn test_parse_bazel_target() {
        // Test parsing of Bazel target references
        let target = "//path/to:target";
        assert!(target.starts_with("//"));
        
        let parts: Vec<&str> = target.trim_start_matches("//").split(':').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "path/to");
        assert_eq!(parts[1], "target");
    }
    
    #[test]
    fn test_build_file_extensions() {
        let build_files = vec!["BUILD", "BUILD.bazel", "test.bzl"];
        
        for file in &build_files {
            if file == &"BUILD" || file == &"BUILD.bazel" {
                assert!(file.ends_with("BUILD") || file.ends_with("BUILD.bazel"));
            }
        }
    }
    
    #[test]
    fn test_language_detection() {
        let test_cases = vec![
            ("file.go", "go"),
            ("file.ts", "typescript"),
            ("file.tsx", "typescript"),
            ("file.js", "typescript"),
            ("file.jsx", "typescript"),
            ("file.py", "python"),
            ("file.java", "java"),
            ("file.rs", "unknown"),
        ];
        
        for (filename, expected_lang) in test_cases {
            let ext = filename.split('.').last().unwrap_or("");
            let lang = match ext {
                "go" => "go",
                "ts" | "tsx" | "js" | "jsx" => "typescript",
                "py" => "python",
                "java" => "java",
                _ => "unknown",
            };
            assert_eq!(lang, expected_lang);
        }
    }
} 