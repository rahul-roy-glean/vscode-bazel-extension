# VSCode Bazel Multi-Language Extension Design Document

## Executive Summary

This document outlines the design for a comprehensive VSCode extension that provides seamless multi-language support for Bazel-based development in the Scio/Glean repository. The extension will eliminate the need to switch between multiple IDEs by providing unified build, test, debug, and code navigation capabilities for Go, TypeScript, Python, and Java within VSCode.

## Problem Statement

Current challenges with Bazel development in VSCode:
- Existing Bazel extensions have broken functionality and poor code navigation
- Developers must switch between different IDEs for different languages
- No unified debugging experience across languages
- Poor integration with custom Bazel rules and tooling
- Difficult to navigate complex cross-language dependencies
- Limited visibility into Bazel build/test outputs

## Goals

### Primary Goals
1. **Unified Development Experience**: Single IDE for all languages in the monorepo
2. **Accurate Code Navigation**: Go-to-definition, find references, and symbol search that understand Bazel's build graph
3. **Integrated Debugging**: Debug any language directly from VSCode with proper source mapping
4. **Build System Integration**: Execute Bazel commands with proper visualization and error reporting
5. **Test Discovery & Execution**: Run and debug tests for all languages with inline results

### Non-Goals
- Replacing Bazel as the build system
- Supporting non-Bazel projects
- Providing language-specific features beyond what's needed for Bazel integration
- Managing DevDock or Docker containers (separate concern)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        VSCode Extension                          │
├─────────────────────────────────────────────────────────────────┤
│                      Extension Host Process                      │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │   UI Layer  │  │ Command      │  │ Language Service       │ │
│  │ - TreeViews │  │ Registry     │  │ Coordinator            │ │
│  │ - CodeLens  │  │ - Build/Test │  │ - Route to providers   │ │
│  │ - Diagnostics│ │ - Debug      │  │ - Merge results        │ │
│  └─────────────┘  └──────────────┘  └────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                    Core Services Layer                           │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │Bazel Client │  │ Build Graph  │  │ Workspace Scanner      │ │
│  │- Query API  │  │ Analyzer     │  │ - Find BUILD files     │ │
│  │- Build/Test │  │- Deps mapping│  │ - Track changes        │ │
│  └─────────────┘  └──────────────┘  └────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│              Language-Specific Providers                         │
│  ┌──────────┐  ┌──────────────┐  ┌─────────┐  ┌─────────────┐ │
│  │   Go     │  │  TypeScript  │  │ Python  │  │    Java     │ │
│  │ Provider │  │   Provider   │  │Provider │  │  Provider   │ │
│  └──────────┘  └──────────────┘  └─────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                 External Processes/Tools                         │
│  ┌──────────┐  ┌──────────────┐  ┌─────────┐  ┌─────────────┐ │
│  │  gopls   │  │TS Language  │  │ pylsp/  │  │    jdtls    │ │
│  │          │  │   Server     │  │ pyright │  │             │ │
│  └──────────┘  └──────────────┘  └─────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Detailed Component Design

### 1. Bazel Integration Layer

#### 1.1 Bazel Client Service
- **Purpose**: Interface with Bazel command-line tool
- **Key Features**:
  - Execute `bazel query` for dependency analysis
  - Run `bazel build`, `bazel test` with streaming output
  - Parse Bazel's Build Event Protocol (BEP) for detailed results
  - Cache query results for performance
  - Handle custom Scio Bazel rules (scio_java_test, etc.)

#### 1.2 Build Graph Analyzer
- **Purpose**: Understand project structure and dependencies
- **Key Features**:
  - Parse BUILD.bazel files to extract targets
  - Build dependency graph across languages
  - Map source files to Bazel targets
  - Provide inverse mapping (which targets use this file)
  - Track generated files and their sources

### 2. Language Service Coordination

#### 2.1 Unified Language Service
- **Purpose**: Coordinate language-specific providers
- **Key Features**:
  - Route requests to appropriate language provider
  - Merge cross-language references (e.g., proto definitions)
  - Handle Bazel-specific imports and path resolution
  - Provide workspace-wide symbol search

#### 2.2 Path Resolution
- **Purpose**: Resolve Bazel-style imports to file system paths
- **Key Features**:
  - Handle Bazel workspace-relative imports
  - Resolve generated file locations
  - Support custom import patterns per language
  - Cache resolution results

### 3. Language-Specific Providers

#### 3.1 Go Provider
- **Integration**: gopls with custom configuration
- **Bazel-Specific Features**:
  - Configure GOPATH based on Bazel's external dependencies
  - Handle generated proto files
  - Support for custom linters in `go/tools/linters/`
  - Debug configuration for Bazel-built binaries

#### 3.2 TypeScript Provider
- **Integration**: TypeScript Language Server
- **Bazel-Specific Features**:
  - Configure path mappings from tsconfig.json
  - Handle webpack aliases and Bazel outputs
  - Support for both web and extension development
  - Integration with pnpm workspace

#### 3.3 Python Provider
- **Integration**: pylsp or Pyright
- **Bazel-Specific Features**:
  - Configure Python path based on Bazel's hermetic Python
  - Handle requirements and external dependencies
  - Support for Bazel-managed virtual environments
  - Debug configuration for DevDock integration

#### 3.4 Java Provider
- **Integration**: Eclipse JDT Language Server
- **Bazel-Specific Features**:
  - Configure classpath from Bazel query
  - Handle generated sources (proto, etc.)
  - Support for custom test rules
  - Integration with Maven dependencies

### 4. User Interface Components

#### 4.1 Bazel Targets View
- Tree view showing all Bazel targets
- Grouped by package and type
- Quick actions: Build, Test, Debug
- Show target dependencies

#### 4.2 Test Explorer Integration
- Discover tests from BUILD files
- Run individual tests or test suites
- Show inline test results
- Support debugging tests

#### 4.3 Build Output Panel
- Structured view of build/test results
- Click-through to errors in source
- Build progress visualization
- Filter by severity/target

#### 4.4 CodeLens Providers
- Show "Build", "Test", "Debug" above targets
- Display dependency count
- Quick navigation to BUILD file

### 5. Command Implementation

#### 5.1 Build Commands
```typescript
interface BuildCommands {
  'bazel.buildTarget': (target: string) => Promise<BuildResult>;
  'bazel.buildFile': (file: string) => Promise<BuildResult>;
  'bazel.buildAll': () => Promise<BuildResult>;
  'bazel.clean': () => Promise<void>;
}
```

#### 5.2 Test Commands
```typescript
interface TestCommands {
  'bazel.testTarget': (target: string) => Promise<TestResult>;
  'bazel.testFile': (file: string) => Promise<TestResult>;
  'bazel.testAll': () => Promise<TestResult>;
  'bazel.debugTest': (target: string) => Promise<void>;
}
```

#### 5.3 Navigation Commands
```typescript
interface NavigationCommands {
  'bazel.goToTarget': (target: string) => Promise<void>;
  'bazel.findTargetForFile': (file: string) => Promise<string[]>;
  'bazel.showDependencies': (target: string) => Promise<void>;
}
```

## Implementation Strategy

### Phase 1: Core Infrastructure (Weeks 1-4)
1. **Bazel Client Service**: Basic query, build, test execution
2. **Workspace Scanner**: Find and parse BUILD files
3. **Basic UI**: Target tree view, output panel
4. **Single Language PoC**: Start with Go integration

### Phase 2: Language Support (Weeks 5-8)
1. **Go Provider**: Complete integration with gopls
2. **TypeScript Provider**: TS server with path resolution
3. **Python Provider**: pylsp/pyright integration
4. **Java Provider**: Basic JDT LS integration

### Phase 3: Advanced Features (Weeks 9-12)
1. **Cross-Language Navigation**: Proto definitions, etc.
2. **Debugging Support**: All languages
3. **Test Explorer**: Full integration
4. **Performance Optimization**: Caching, incremental updates

### Phase 4: Developer Experience (Weeks 13-16)
1. **Custom Rule Support**: Handle Scio-specific rules
2. **Quick Fixes**: Auto-add dependencies
3. **Refactoring Support**: Update BUILD files
4. **Documentation**: User guide and API docs

## Technical Considerations

### Performance
- **Lazy Loading**: Only analyze visible/active parts of workspace
- **Caching**: Cache Bazel query results, build graph
- **Incremental Updates**: React to file changes efficiently
- **Background Processing**: Don't block UI for analysis

### Compatibility
- **VSCode Version**: Target latest stable (1.80+)
- **Bazel Version**: Support 6.0+ (used in Scio)
- **OS Support**: macOS, Linux (Windows secondary)
- **Language Versions**: Match Scio's requirements

### Configuration
```json
{
  "bazel.executable": "/usr/local/bin/bazel",
  "bazel.workspaceRoot": "${workspaceFolder}",
  "bazel.buildFlags": ["--config=dev"],
  "bazel.testFlags": ["--test_output=errors"],
  "bazel.languages": {
    "go": {
      "enabled": true,
      "goplsPath": "auto"
    },
    "typescript": {
      "enabled": true,
      "tsserverPath": "auto"
    },
    "python": {
      "enabled": true,
      "interpreter": "bazel-bin/python_scio/scio_env/bin/python"
    },
    "java": {
      "enabled": true,
      "jdtlsPath": "auto"
    }
  }
}
```

## Testing Strategy

### Unit Tests
- Test each service in isolation
- Mock Bazel CLI interactions
- Test language provider integration

### Integration Tests
- Test against sample Bazel workspaces
- Verify cross-language features
- Test with Scio-specific rules

### End-to-End Tests
- Test complete workflows
- Verify UI interactions
- Performance benchmarks

## Success Metrics

1. **Developer Productivity**
   - Time to navigate between related files reduced by 50%
   - Build/test cycle time reduced by 30%
   - Debugging setup time reduced from minutes to seconds

2. **Adoption**
   - 80% of developers using VSCode as primary IDE
   - Reduced context switching between IDEs

3. **Quality**
   - Code navigation accuracy > 95%
   - Build/test reliability > 99%
   - Extension startup time < 5 seconds

## Risks and Mitigations

### Technical Risks
1. **Bazel API Changes**
   - Mitigation: Abstract Bazel interface, version detection
   
2. **Language Server Conflicts**
   - Mitigation: Isolated server processes, clear precedence rules

3. **Performance at Scale**
   - Mitigation: Lazy loading, intelligent caching

### Adoption Risks
1. **Learning Curve**
   - Mitigation: Comprehensive docs, migration guide

2. **Feature Parity**
   - Mitigation: Phased rollout, gather feedback early

## Open Questions

1. **Integration with DevDock**: Should the extension manage DevDock services?
2. **Remote Development**: Support for remote development scenarios?
3. **AI Integration**: Incorporate AI-assisted development features?
4. **Custom Linters**: How deep should integration with custom tools go?

## Conclusion

This VSCode extension will provide a unified, efficient development experience for the Scio/Glean monorepo. By deeply integrating with Bazel and providing intelligent language support, developers can work productively across all languages without switching IDEs. The phased implementation approach ensures we can deliver value incrementally while building toward a comprehensive solution.