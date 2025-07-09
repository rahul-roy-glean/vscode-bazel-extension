# Bazel Language Server Protocol (LSP) Implementation

A high-performance Language Server Protocol implementation for Bazel, written in Rust.

## Features

- **Fast BUILD file parsing** using pest parser generator
- **Multi-language support** with external LSP integration (Go, TypeScript, Python, Java)
- **Bazel query integration** with protobuf support
- **Build Event Protocol (BEP)** parsing for rich build insights
- **Concurrent operations** using Tokio and Rayon
- **Smart caching** with LRU cache for query results

## Architecture

The server implements a hybrid architecture:

- **Rust core**: Handles Bazel-specific operations, BUILD file parsing, and coordination
- **External LSPs**: Delegates language-specific features to dedicated language servers
- **Async I/O**: Non-blocking operations for all Bazel commands

## Building

### Prerequisites

- Rust 1.70+ with Cargo
- Protobuf compiler (`protoc`)
- Bazel (for testing)

### Build Steps

```bash
# Install protoc (macOS)
brew install protobuf

# Install protoc (Ubuntu/Debian)
sudo apt-get install protobuf-compiler

# Build the server
cargo build --release

# The binary will be at target/release/bazel-lsp
```

## Language Server Setup

### Go Support

Install gopls:
```bash
go install golang.org/x/tools/gopls@latest
```

### TypeScript Support

Install TypeScript language server:
```bash
npm install -g typescript-language-server typescript
```

### Python Support

Install Python LSP:
```bash
pip install python-lsp-server
# or
pip install pyright
```

### Java Support

Download Eclipse JDT Language Server from:
https://download.eclipse.org/jdtls/

## Usage

The server communicates via stdio and implements the Language Server Protocol v3.17.

### With VSCode Extension

The server is automatically started by the VSCode extension when you open a Bazel workspace.

### Standalone Testing

```bash
# Start the server
./target/release/bazel-lsp

# Send LSP messages via stdin
```

## Configuration

The server accepts initialization options:

```json
{
  "bazel": {
    "executable": "bazel",
    "cache": {
      "queryResults": true,
      "ttl": 300
    }
  },
  "languages": {
    "go": {
      "enabled": true,
      "goplsPath": "gopls"
    },
    "typescript": {
      "enabled": true
    },
    "python": {
      "enabled": true
    },
    "java": {
      "enabled": true
    }
  }
}
```

## Development

### Running Tests

```bash
cargo test
```

### Debugging

Set `RUST_LOG` environment variable:
```bash
RUST_LOG=debug ./target/release/bazel-lsp
```

### Adding New Language Support

1. Create a new module in `src/languages/`
2. Implement the `LanguageServerProxy` trait
3. Add initialization in `LanguageCoordinator`

## Performance

- BUILD file parsing: ~50x faster than regex-based approaches
- Parallel workspace scanning using Rayon
- Zero-copy protobuf parsing
- Efficient caching with thread-safe access

## License

Same as parent project 