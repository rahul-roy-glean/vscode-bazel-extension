# Contributing to VSCode Bazel Extension

Thank you for your interest in contributing to the VSCode Bazel Extension! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Code Style](#code-style)
- [Submitting Changes](#submitting-changes)
- [Release Process](#release-process)

## Development Setup

### Prerequisites

1. **System Requirements**
   - Node.js 16+ and npm 8+
   - Rust 1.70+ and Cargo
   - VSCode 1.80+
   - Bazel 6.0+
   - Git

2. **Language-Specific Tools** (for testing)
   - Go and gopls
   - Python 3.8+ and pylsp
   - Java 11+ and Maven
   - TypeScript (installed via npm)

### Initial Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/rahul-roy-glean/vscode-bazel-extension.git
   cd vscode-bazel-extension
   ```

2. **Install dependencies**
   ```bash
   # Install Node dependencies
   npm install
   
   # Build Rust components
   cd bazel-lsp
   cargo build --release
   cd ..
   ```

3. **Set up pre-commit hooks**
   ```bash
   npm run setup-hooks
   ```

### Development Environment

1. **Open in VSCode**
   ```bash
   code .
   ```

2. **Recommended Extensions**
   - rust-analyzer
   - ESLint
   - Prettier
   - EditorConfig

3. **Launch Configurations**
   The project includes launch configurations for debugging:
   - `Extension` - Launch extension in new VSCode window
   - `Extension + Server` - Launch with Rust LSP debugging
   - `Tests` - Run extension tests

## Project Structure

```
vscode-bazel-extension/
├── extension/                    # TypeScript VSCode extension
│   ├── src/
│   │   ├── extension.ts         # Extension entry point
│   │   ├── commands/            # Command implementations
│   │   ├── providers/           # VSCode providers
│   │   └── client/              # LSP client
│   ├── test/                    # Extension tests
│   └── package.json
├── bazel-lsp/                   # Rust language server
│   ├── src/
│   │   ├── main.rs             # Server entry point
│   │   ├── server.rs           # LSP implementation
│   │   ├── bazel/              # Bazel-specific logic
│   │   └── languages/          # Language handlers
│   ├── tests/                  # Server tests
│   └── Cargo.toml
├── docs/                        # Documentation
│   ├── design/                 # Design documents
│   └── api/                    # API documentation
└── scripts/                     # Build and utility scripts
```

## Development Workflow

### Running the Extension

1. **Quick Development (TypeScript only)**
   ```bash
   # Terminal 1: Watch TypeScript changes
   npm run watch
   
   # In VSCode: Press F5 to launch extension
   ```

2. **Full Development (TypeScript + Rust)**
   ```bash
   # Terminal 1: Build and watch Rust server
   cd bazel-lsp
   cargo watch -x "build --release" -s "cp target/release/bazel-lsp ../extension/server/"
   
   # Terminal 2: Watch TypeScript changes
   npm run watch
   
   # In VSCode: Press F5 to launch extension
   ```

3. **Debug Mode**
   ```bash
   # Set environment variable for verbose logging
   export RUST_LOG=debug
   
   # Launch "Extension + Server" configuration from VSCode
   ```

### Making Changes

1. **TypeScript Extension Changes**
   - Edit files in `extension/src/`
   - Changes hot-reload in the development instance
   - Use VSCode's debugger for breakpoints

2. **Rust Server Changes**
   - Edit files in `bazel-lsp/src/`
   - Server must be rebuilt and restarted
   - Use `RUST_LOG` for debugging output

3. **Adding New Commands**
   ```typescript
   // extension/src/commands/myCommand.ts
   export async function executeMyCommand() {
     // Implementation
   }
   
   // Register in extension.ts
   commands.registerCommand('bazel.myCommand', executeMyCommand)
   ```

4. **Adding LSP Features**
   ```rust
   // bazel-lsp/src/server.rs
   #[tower_lsp::async_trait]
   impl LanguageServer for BazelLanguageServer {
     async fn my_feature(&self, params: MyParams) -> Result<MyResponse> {
       // Implementation
     }
   }
   ```

## Testing

### Running Tests

```bash
# Run all tests
npm test

# Run TypeScript tests only
npm run test:extension

# Run Rust tests only
npm run test:server

# Run integration tests
npm run test:integration
```

### Writing Tests

1. **Extension Tests** (TypeScript)
   ```typescript
   // extension/test/commands.test.ts
   import * as assert from 'assert';
   import * as vscode from 'vscode';
   
   suite('Bazel Commands', () => {
     test('Build command executes', async () => {
       const result = await vscode.commands.executeCommand('bazel.build');
       assert.ok(result);
     });
   });
   ```

2. **Server Tests** (Rust)
   ```rust
   // bazel-lsp/tests/parser_test.rs
   #[test]
   fn test_parse_build_file() {
     let content = r#"
       go_binary(
         name = "server",
         srcs = ["main.go"],
       )
     "#;
     
     let rules = parse_build_file(content).unwrap();
     assert_eq!(rules.len(), 1);
     assert_eq!(rules[0].name, "server");
   }
   ```

3. **Integration Tests**
   - Located in `test/integration/`
   - Test full extension functionality
   - Use test workspaces in `test/fixtures/`

### Test Coverage

- Aim for >80% code coverage
- Run coverage report: `npm run coverage`
- View HTML report: `open coverage/index.html`

## Code Style

### TypeScript Style

- Follow ESLint configuration
- Use Prettier for formatting
- Naming conventions:
  - Files: camelCase.ts
  - Classes: PascalCase
  - Functions/variables: camelCase
  - Constants: UPPER_SNAKE_CASE

### Rust Style

- Follow rustfmt configuration
- Use clippy for linting
- Naming conventions:
  - Files: snake_case.rs
  - Types: PascalCase
  - Functions/variables: snake_case
  - Constants: UPPER_SNAKE_CASE

### General Guidelines

1. **Comments**
   - Document public APIs
   - Explain "why", not "what"
   - Use TODO/FIXME with issue numbers

2. **Error Handling**
   - TypeScript: Use try/catch, return Result types
   - Rust: Use Result<T, E>, avoid unwrap() in production

3. **Logging**
   - TypeScript: Use output channels
   - Rust: Use tracing crate

## Submitting Changes

### Pull Request Process

1. **Create a Feature Branch**
   ```bash
   git checkout -b feature/my-feature
   ```

2. **Make Your Changes**
   - Write tests for new functionality
   - Update documentation
   - Follow code style guidelines

3. **Commit Guidelines**
   ```bash
   # Format: <type>(<scope>): <subject>
   git commit -m "feat(parser): add support for new rule types"
   ```
   
   Types:
   - `feat`: New feature
   - `fix`: Bug fix
   - `docs`: Documentation
   - `style`: Code style changes
   - `refactor`: Code refactoring
   - `test`: Test changes
   - `chore`: Build/tooling changes

4. **Push and Create PR**
   ```bash
   git push origin feature/my-feature
   # Create PR on GitHub
   ```

5. **PR Requirements**
   - Descriptive title and description
   - Link related issues
   - All tests passing
   - Code review approval
   - Documentation updated

### Code Review

- Address all feedback promptly
- Keep discussions professional
- Update PR based on feedback
- Squash commits before merging

## Release Process

### Version Numbering

Follow [Semantic Versioning](https://semver.org/):
- MAJOR: Breaking changes
- MINOR: New features (backward compatible)
- PATCH: Bug fixes

### Release Steps

1. **Update Version**
   ```bash
   npm version minor  # or major, patch
   ```

2. **Update CHANGELOG.md**
   - List all changes
   - Credit contributors
   - Note breaking changes

3. **Build Release**
   ```bash
   npm run package
   ```

4. **Test Release**
   - Install .vsix locally
   - Run smoke tests
   - Verify on different platforms

5. **Create GitHub Release**
   - Tag with version
   - Attach .vsix file
   - Copy changelog

6. **Publish to Marketplace**
   ```bash
   vsce publish
   ```

## Getting Help

- **Discord**: Join #vscode-bazel-dev channel
- **Issues**: Check existing issues or create new ones
- **Wiki**: Development tips and tricks
- **Office Hours**: Thursdays 2-3pm PT

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.