# VSCode Bazel Extension Implementation Guide

## Overview

This guide provides detailed implementation guidance and code examples for building the VSCode Bazel Multi-Language Extension. It supplements the design document with concrete technical details.

## Core Extension Structure

### Project Layout
```
vscode-bazel-extension/
├── src/
│   ├── extension.ts              # Extension entry point
│   ├── bazel/
│   │   ├── client.ts            # Bazel CLI wrapper
│   │   ├── query.ts             # Bazel query operations
│   │   ├── buildGraph.ts        # Build graph analyzer
│   │   └── rules/
│   │       └── scio.ts          # Scio-specific rule handlers
│   ├── languages/
│   │   ├── coordinator.ts       # Language service coordinator
│   │   ├── go/
│   │   │   ├── provider.ts      # Go language provider
│   │   │   └── debugger.ts      # Go debug adapter
│   │   ├── typescript/
│   │   ├── python/
│   │   └── java/
│   ├── ui/
│   │   ├── treeViews/
│   │   ├── codeLens/
│   │   └── panels/
│   └── utils/
│       ├── cache.ts
│       └── workspace.ts
├── package.json
├── tsconfig.json
└── webpack.config.js
```

## Key Implementation Details

### 1. Bazel Client Implementation

```typescript
// src/bazel/client.ts
import { spawn } from 'child_process';
import * as vscode from 'vscode';

export interface BazelQueryResult {
  targets: string[];
  dependencies: Map<string, string[]>;
}

export class BazelClient {
  private readonly workspaceRoot: string;
  private readonly bazelExecutable: string;
  
  constructor(workspaceRoot: string) {
    this.workspaceRoot = workspaceRoot;
    this.bazelExecutable = vscode.workspace
      .getConfiguration('bazel')
      .get('executable', 'bazel');
  }

  async query(query: string): Promise<BazelQueryResult> {
    const args = [
      'query',
      query,
      '--output=proto',
      '--proto:output_rule_attrs=srcs,deps',
    ];
    
    const result = await this.executeBazel(args);
    return this.parseQueryResult(result);
  }

  async build(target: string, options?: BuildOptions): Promise<BuildResult> {
    const args = ['build', target];
    
    if (options?.flags) {
      args.push(...options.flags);
    }
    
    // Use Build Event Protocol for detailed results
    const bepFile = path.join(os.tmpdir(), `bazel-bep-${Date.now()}.json`);
    args.push(`--build_event_json_file=${bepFile}`);
    
    try {
      await this.executeBazel(args, { streaming: true });
      return await this.parseBuildEventProtocol(bepFile);
    } finally {
      await fs.unlink(bepFile);
    }
  }

  private async executeBazel(
    args: string[], 
    options?: { streaming?: boolean }
  ): Promise<string> {
    return new Promise((resolve, reject) => {
      const proc = spawn(this.bazelExecutable, args, {
        cwd: this.workspaceRoot,
        shell: true,
      });

      let stdout = '';
      let stderr = '';

      proc.stdout.on('data', (data) => {
        stdout += data;
        if (options?.streaming) {
          this.outputChannel.append(data.toString());
        }
      });

      proc.stderr.on('data', (data) => {
        stderr += data;
        if (options?.streaming) {
          this.outputChannel.append(data.toString());
        }
      });

      proc.on('close', (code) => {
        if (code === 0) {
          resolve(stdout);
        } else {
          reject(new Error(`Bazel failed: ${stderr}`));
        }
      });
    });
  }
}
```

### 2. Build Graph Analyzer

```typescript
// src/bazel/buildGraph.ts
import * as parser from '@bazel/buildtools';

export interface BazelTarget {
  name: string;
  kind: string;
  sources: string[];
  dependencies: string[];
  location: vscode.Location;
}

export class BuildGraphAnalyzer {
  private targetCache = new Map<string, BazelTarget>();
  private fileToTargets = new Map<string, string[]>();
  
  async analyzeWorkspace(workspaceRoot: string): Promise<void> {
    const buildFiles = await this.findBuildFiles(workspaceRoot);
    
    for (const buildFile of buildFiles) {
      await this.parseBuildFile(buildFile);
    }
  }

  async parseBuildFile(filePath: string): Promise<void> {
    const content = await fs.readFile(filePath, 'utf8');
    const ast = parser.parse(content);
    
    // Extract targets from AST
    const targets = this.extractTargets(ast, filePath);
    
    for (const target of targets) {
      this.targetCache.set(target.name, target);
      
      // Update file-to-target mapping
      for (const source of target.sources) {
        const absPath = path.resolve(path.dirname(filePath), source);
        if (!this.fileToTargets.has(absPath)) {
          this.fileToTargets.set(absPath, []);
        }
        this.fileToTargets.get(absPath)!.push(target.name);
      }
    }
  }

  private extractTargets(ast: any, filePath: string): BazelTarget[] {
    const targets: BazelTarget[] = [];
    
    // Walk AST to find rule calls
    ast.walkAll((node: any) => {
      if (node.type === 'call' && this.isBazelRule(node.name)) {
        const target = this.parseRuleCall(node, filePath);
        if (target) {
          targets.push(target);
        }
      }
    });
    
    return targets;
  }

  private parseRuleCall(node: any, filePath: string): BazelTarget | null {
    const name = this.getAttributeValue(node, 'name');
    if (!name) return null;
    
    const packageName = this.getPackageFromPath(filePath);
    
    return {
      name: `//${packageName}:${name}`,
      kind: node.name,
      sources: this.getListAttribute(node, 'srcs') || [],
      dependencies: this.getListAttribute(node, 'deps') || [],
      location: new vscode.Location(
        vscode.Uri.file(filePath),
        new vscode.Position(node.line - 1, node.column - 1)
      ),
    };
  }

  getTargetsForFile(filePath: string): string[] {
    return this.fileToTargets.get(filePath) || [];
  }

  getTarget(targetName: string): BazelTarget | undefined {
    return this.targetCache.get(targetName);
  }
}
```

### 3. Language Service Coordinator

```typescript
// src/languages/coordinator.ts
export class LanguageServiceCoordinator {
  private providers = new Map<string, LanguageProvider>();
  private buildGraph: BuildGraphAnalyzer;
  
  constructor(buildGraph: BuildGraphAnalyzer) {
    this.buildGraph = buildGraph;
    this.initializeProviders();
  }

  private initializeProviders() {
    this.providers.set('go', new GoProvider(this.buildGraph));
    this.providers.set('typescript', new TypeScriptProvider(this.buildGraph));
    this.providers.set('python', new PythonProvider(this.buildGraph));
    this.providers.set('java', new JavaProvider(this.buildGraph));
  }

  async provideDefinition(
    document: vscode.TextDocument,
    position: vscode.Position
  ): Promise<vscode.LocationLink[]> {
    const provider = this.getProviderForDocument(document);
    if (!provider) return [];
    
    // Get language-specific definitions
    const definitions = await provider.provideDefinition(document, position);
    
    // Check for cross-language references (e.g., proto imports)
    const crossLangDefs = await this.resolveCrossLanguageReferences(
      document,
      position
    );
    
    return [...definitions, ...crossLangDefs];
  }

  private async resolveCrossLanguageReferences(
    document: vscode.TextDocument,
    position: vscode.Position
  ): Promise<vscode.LocationLink[]> {
    const line = document.lineAt(position);
    const text = line.text;
    
    // Example: Handle proto imports across languages
    const protoImportMatch = text.match(/import\s+"(.+\.proto)"/);
    if (protoImportMatch) {
      const protoPath = protoImportMatch[1];
      return this.resolveProtoImport(protoPath);
    }
    
    // Handle Bazel target references
    const targetMatch = text.match(/["']\/\/([^"']+)["']/);
    if (targetMatch) {
      const targetName = `//${targetMatch[1]}`;
      const target = this.buildGraph.getTarget(targetName);
      if (target) {
        return [{
          targetRange: new vscode.Range(position, position),
          targetUri: target.location.uri,
          targetSelectionRange: target.location.range,
        }];
      }
    }
    
    return [];
  }
}
```

### 4. Go Language Provider

```typescript
// src/languages/go/provider.ts
export class GoProvider implements LanguageProvider {
  private client: LanguageClient;
  private buildGraph: BuildGraphAnalyzer;
  
  constructor(buildGraph: BuildGraphAnalyzer) {
    this.buildGraph = buildGraph;
    this.initializeGopls();
  }

  private async initializeGopls() {
    // Configure gopls for Bazel
    const serverOptions: ServerOptions = {
      command: 'gopls',
      args: ['-mode=stdio'],
    };

    const clientOptions: LanguageClientOptions = {
      documentSelector: [{ scheme: 'file', language: 'go' }],
      initializationOptions: {
        // Configure for Bazel workspace
        'build.directoryFilters': ['-bazel-*'],
        'build.expandWorkspaceToModule': false,
        'build.experimentalWorkspaceModule': true,
      },
      middleware: {
        // Intercept go-to-definition to handle Bazel imports
        provideDefinition: async (document, position, token, next) => {
          const result = await next(document, position, token);
          return this.enhanceDefinitionWithBazel(document, position, result);
        },
      },
    };

    this.client = new LanguageClient(
      'go-bazel',
      'Go (Bazel)',
      serverOptions,
      clientOptions
    );

    await this.client.start();
  }

  private async enhanceDefinitionWithBazel(
    document: vscode.TextDocument,
    position: vscode.Position,
    result: vscode.Definition | vscode.LocationLink[] | null
  ): Promise<vscode.LocationLink[]> {
    if (result) return Array.isArray(result) ? result : [result];
    
    // If gopls couldn't resolve, try Bazel-specific resolution
    const line = document.lineAt(position).text;
    const importMatch = line.match(/import\s+"(.+)"/);
    
    if (importMatch) {
      const importPath = importMatch[1];
      const resolvedPath = await this.resolveGoImport(importPath);
      if (resolvedPath) {
        return [{
          targetUri: vscode.Uri.file(resolvedPath),
          targetRange: new vscode.Range(0, 0, 0, 0),
        }];
      }
    }
    
    return [];
  }

  private async resolveGoImport(importPath: string): Promise<string | null> {
    // Handle Bazel-style imports
    if (importPath.startsWith('github.com/askscio/scio/')) {
      const relativePath = importPath.replace('github.com/askscio/scio/', '');
      return path.join(this.workspaceRoot, relativePath);
    }
    
    // Handle generated proto imports
    if (importPath.includes('/proto/')) {
      const bazelBin = path.join(this.workspaceRoot, 'bazel-bin');
      const protoPath = importPath.replace(/^.*\/proto\//, 'proto/');
      const generatedPath = path.join(bazelBin, protoPath, 'go_default_library');
      
      if (await fs.pathExists(generatedPath)) {
        return generatedPath;
      }
    }
    
    return null;
  }
}
```

### 5. Test Discovery and Execution

```typescript
// src/testing/testDiscovery.ts
export class BazelTestDiscovery {
  private testController: vscode.TestController;
  private buildGraph: BuildGraphAnalyzer;
  
  constructor(buildGraph: BuildGraphAnalyzer) {
    this.buildGraph = buildGraph;
    this.testController = vscode.tests.createTestController(
      'bazelTests',
      'Bazel Tests'
    );
    
    this.setupTestController();
  }

  private setupTestController() {
    this.testController.resolveHandler = async (item) => {
      if (!item) {
        // Discover all tests in workspace
        await this.discoverAllTests();
      } else {
        // Resolve children of a test item
        await this.resolveTestItem(item);
      }
    };

    this.testController.createRunProfile(
      'Run',
      vscode.TestRunProfileKind.Run,
      (request, token) => this.runTests(request, token)
    );

    this.testController.createRunProfile(
      'Debug',
      vscode.TestRunProfileKind.Debug,
      (request, token) => this.debugTests(request, token)
    );
  }

  private async discoverAllTests() {
    const testTargets = await this.findTestTargets();
    
    for (const target of testTargets) {
      const testItem = this.testController.createTestItem(
        target.name,
        this.getTestLabel(target),
        target.location.uri
      );
      
      testItem.canResolveChildren = true;
      this.testController.items.add(testItem);
    }
  }

  private async findTestTargets(): Promise<BazelTarget[]> {
    // Query for all test targets
    const result = await this.bazelClient.query(
      'kind(".*_test", //...)'
    );
    
    return result.targets
      .map(name => this.buildGraph.getTarget(name))
      .filter(Boolean) as BazelTarget[];
  }

  private async runTests(
    request: vscode.TestRunRequest,
    token: vscode.CancellationToken
  ) {
    const run = this.testController.createTestRun(request);
    const tests = request.include ?? this.testController.items;
    
    for (const test of tests) {
      if (token.isCancellationRequested) break;
      
      run.started(test);
      
      try {
        const result = await this.runBazelTest(test.id);
        
        if (result.success) {
          run.passed(test, result.duration);
        } else {
          const message = new vscode.TestMessage(result.error || 'Test failed');
          message.location = new vscode.Location(
            test.uri!,
            new vscode.Position(0, 0)
          );
          run.failed(test, message, result.duration);
        }
      } catch (error) {
        run.errored(test, new vscode.TestMessage(error.message));
      }
    }
    
    run.end();
  }

  private async runBazelTest(target: string): Promise<TestResult> {
    const startTime = Date.now();
    
    try {
      const result = await this.bazelClient.test(target, {
        flags: ['--test_output=errors'],
      });
      
      return {
        success: result.success,
        duration: Date.now() - startTime,
        output: result.output,
      };
    } catch (error) {
      return {
        success: false,
        duration: Date.now() - startTime,
        error: error.message,
      };
    }
  }
}
```

### 6. Debug Adapter Integration

```typescript
// src/debugging/debugAdapter.ts
export class BazelDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
  private buildGraph: BuildGraphAnalyzer;
  
  async createDebugAdapterDescriptor(
    session: vscode.DebugSession
  ): Promise<vscode.DebugAdapterDescriptor> {
    const config = session.configuration as BazelDebugConfiguration;
    
    // Determine language from target
    const target = this.buildGraph.getTarget(config.target);
    if (!target) {
      throw new Error(`Target ${config.target} not found`);
    }
    
    const language = this.detectLanguage(target);
    
    switch (language) {
      case 'go':
        return this.createGoDebugAdapter(session, target);
      case 'python':
        return this.createPythonDebugAdapter(session, target);
      case 'java':
        return this.createJavaDebugAdapter(session, target);
      default:
        throw new Error(`Debugging not supported for ${language}`);
    }
  }

  private async createGoDebugAdapter(
    session: vscode.DebugSession,
    target: BazelTarget
  ): Promise<vscode.DebugAdapterDescriptor> {
    // Build the target with debug info
    await this.bazelClient.build(target.name, {
      flags: ['--compilation_mode=dbg'],
    });
    
    // Find the binary output
    const binaryPath = await this.findBinaryOutput(target);
    
    // Configure delve
    const debugConfig = {
      name: session.name,
      type: 'go',
      request: 'launch',
      mode: 'exec',
      program: binaryPath,
      env: {
        // Set up Bazel runtime environment
        'RUNFILES_DIR': path.join(path.dirname(binaryPath), `${target.name}.runfiles`),
      },
      args: session.configuration.args || [],
    };
    
    // Use the standard Go debug adapter
    return new vscode.DebugAdapterExecutable('dlv', ['dap']);
  }

  private async createPythonDebugAdapter(
    session: vscode.DebugSession,
    target: BazelTarget
  ): Promise<vscode.DebugAdapterDescriptor> {
    // For Python in DevDock, handle remote debugging
    if (session.configuration.remote) {
      return new vscode.DebugAdapterServer(4329); // DevDock debug port
    }
    
    // Local Python debugging
    const pythonPath = await this.findBazelPython();
    
    return new vscode.DebugAdapterExecutable(pythonPath, [
      '-m',
      'debugpy.adapter',
    ]);
  }
}
```

### 7. Custom Bazel Rules Support

```typescript
// src/bazel/rules/scio.ts
export interface ScioRuleHandler {
  isApplicable(ruleName: string): boolean;
  enhanceTarget(target: BazelTarget): Promise<void>;
  provideCodeLens(target: BazelTarget): vscode.CodeLens[];
}

export class ScioJavaTestHandler implements ScioRuleHandler {
  isApplicable(ruleName: string): boolean {
    return ruleName === 'scio_java_test' || ruleName === 'scio_java_junit5_test';
  }

  async enhanceTarget(target: BazelTarget): Promise<void> {
    // Add Scio-specific attributes
    target.metadata = {
      ...target.metadata,
      testClass: this.extractTestClass(target),
      isBeamTest: target.attributes?.beam_test === true,
    };
  }

  provideCodeLens(target: BazelTarget): vscode.CodeLens[] {
    const lenses: vscode.CodeLens[] = [];
    
    // Standard test lens
    lenses.push(new vscode.CodeLens(target.location.range, {
      title: '▶️ Run Test',
      command: 'bazel.runTest',
      arguments: [target.name],
    }));
    
    // Debug lens
    lenses.push(new vscode.CodeLens(target.location.range, {
      title: '🐛 Debug Test',
      command: 'bazel.debugTest',
      arguments: [target.name],
    }));
    
    // Beam-specific lens
    if (target.metadata?.isBeamTest) {
      lenses.push(new vscode.CodeLens(target.location.range, {
        title: '☁️ Run on Dataflow',
        command: 'bazel.runBeamTest',
        arguments: [target.name],
      }));
    }
    
    return lenses;
  }
}
```

### 8. Configuration Schema

```json
{
  "contributes": {
    "configuration": {
      "title": "Bazel",
      "properties": {
        "bazel.executable": {
          "type": "string",
          "default": "bazel",
          "description": "Path to Bazel executable"
        },
        "bazel.buildFlags": {
          "type": "array",
          "default": [],
          "description": "Default flags for bazel build"
        },
        "bazel.testFlags": {
          "type": "array",
          "default": ["--test_output=errors"],
          "description": "Default flags for bazel test"
        },
        "bazel.enableCodeLens": {
          "type": "boolean",
          "default": true,
          "description": "Show build/test/debug actions inline"
        },
        "bazel.languages.go.enabled": {
          "type": "boolean",
          "default": true,
          "description": "Enable Go language support"
        },
        "bazel.languages.go.goplsPath": {
          "type": "string",
          "default": "gopls",
          "description": "Path to gopls executable"
        },
        "bazel.cache.queryResults": {
          "type": "boolean",
          "default": true,
          "description": "Cache Bazel query results"
        },
        "bazel.cache.ttl": {
          "type": "number",
          "default": 300,
          "description": "Cache TTL in seconds"
        }
      }
    }
  }
}
```

## Performance Optimizations

### 1. Incremental Build Graph Updates

```typescript
// Watch for BUILD file changes
const buildWatcher = vscode.workspace.createFileSystemWatcher('**/BUILD{,.bazel}');

buildWatcher.onDidChange(async (uri) => {
  // Only re-parse the changed file
  await buildGraph.parseBuildFile(uri.fsPath);
  
  // Update affected language servers
  const affected = buildGraph.getAffectedTargets(uri.fsPath);
  for (const target of affected) {
    await languageCoordinator.notifyTargetChanged(target);
  }
});
```

### 2. Query Result Caching

```typescript
class CachedBazelClient extends BazelClient {
  private cache = new LRUCache<string, any>({
    max: 1000,
    ttl: 1000 * 60 * 5, // 5 minutes
  });

  async query(query: string): Promise<BazelQueryResult> {
    const cached = this.cache.get(query);
    if (cached) return cached;
    
    const result = await super.query(query);
    this.cache.set(query, result);
    return result;
  }
}
```

## Integration with Existing Tools

### 1. DevDock Integration

```typescript
// Detect if running in DevDock environment
const isDevDock = await fs.pathExists(path.join(workspaceRoot, 'devdock.yaml'));

if (isDevDock) {
  // Configure Python debugging for DevDock
  vscode.workspace.getConfiguration('python').update('defaultInterpreterPath', 
    'bazel-bin/python_scio/scio_env/bin/python'
  );
  
  // Add DevDock-specific debug configurations
  const launch = vscode.workspace.getConfiguration('launch');
  const configs = launch.get<any[]>('configurations', []);
  
  configs.push({
    name: 'Python: Attach to DevDock',
    type: 'python',
    request: 'attach',
    connect: {
      host: 'localhost',
      port: 4329,
    },
    pathMappings: [{
      localRoot: '${workspaceFolder}/python_scio',
      remoteRoot: '/app/qp/query_parser.runfiles/com_github_askscio_scio/python_scio',
    }],
  });
}
```

### 2. Custom Linter Integration

```typescript
// Register custom Go linters
const linterProvider = new class implements vscode.CodeActionProvider {
  async provideCodeActions(
    document: vscode.TextDocument
  ): Promise<vscode.CodeAction[]> {
    if (document.languageId !== 'go') return [];
    
    // Run Scio custom linters
    const linters = [
      'disallow_param_type_grouping',
      'disallow_print_statements',
      'require_underscore_unused',
    ];
    
    const actions: vscode.CodeAction[] = [];
    
    for (const linter of linters) {
      const result = await bazelClient.run(
        `//go/tools/linters/${linter}`,
        ['--', document.fileName]
      );
      
      if (result.violations) {
        actions.push(...this.createCodeActions(result.violations));
      }
    }
    
    return actions;
  }
};
```

## Testing the Extension

### 1. Unit Test Example

```typescript
// test/bazel/client.test.ts
describe('BazelClient', () => {
  let client: BazelClient;
  let execStub: sinon.SinonStub;
  
  beforeEach(() => {
    execStub = sinon.stub(child_process, 'spawn');
    client = new BazelClient('/workspace');
  });
  
  it('should parse query results correctly', async () => {
    const mockOutput = `
      name: "//go/server:server"
      rule_class: "go_binary"
      attribute {
        name: "srcs"
        string_list_value {
          string_value: "main.go"
          string_value: "server.go"
        }
      }
    `;
    
    execStub.returns({
      stdout: { on: (event: string, cb: Function) => cb(mockOutput) },
      stderr: { on: () => {} },
      on: (event: string, cb: Function) => cb(0),
    });
    
    const result = await client.query('//go/server:server');
    
    expect(result.targets).to.include('//go/server:server');
    expect(result.dependencies.get('//go/server:server')).to.deep.equal([]);
  });
});
```

### 2. Integration Test Example

```typescript
// test/integration/languages.test.ts
describe('Multi-language Integration', () => {
  let extension: vscode.Extension<any>;
  
  before(async () => {
    // Open test workspace
    const workspaceUri = vscode.Uri.file(
      path.join(__dirname, 'fixtures', 'test-workspace')
    );
    await vscode.commands.executeCommand('vscode.openFolder', workspaceUri);
    
    // Activate extension
    extension = vscode.extensions.getExtension('scio.bazel-multilang')!;
    await extension.activate();
  });
  
  it('should navigate from Go to Proto definition', async () => {
    const goFile = await vscode.workspace.openTextDocument(
      path.join(vscode.workspace.rootPath!, 'go/server/main.go')
    );
    
    // Find proto import
    const importLine = goFile.getText().split('\n')
      .findIndex(line => line.includes('proto/api'));
    
    // Trigger go-to-definition
    const definitions = await vscode.commands.executeCommand<vscode.Location[]>(
      'vscode.executeDefinitionProvider',
      goFile.uri,
      new vscode.Position(importLine, 20)
    );
    
    expect(definitions).to.have.lengthOf(1);
    expect(definitions[0].uri.fsPath).to.include('proto/api/api.proto');
  });
});
```

## Conclusion

This implementation guide provides the technical foundation for building the VSCode Bazel Multi-Language Extension. The modular architecture allows for incremental development while maintaining extensibility for future enhancements.