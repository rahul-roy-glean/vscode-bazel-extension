import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';
import { registerCommands } from './commands';
import { BazelTargetProvider } from './providers/targetProvider';

let client: LanguageClient;

export async function activate(context: vscode.ExtensionContext) {
    // Show a message immediately to confirm activation
    vscode.window.showInformationMessage('Bazel extension is activating...');
    console.log('Bazel extension is now active!');

    try {
        // Path to the Rust LSP binary
        const serverModule = context.asAbsolutePath(
            path.join('server', process.platform === 'win32' ? 'bazel-lsp.exe' : 'bazel-lsp')
        );

        // Check if the server binary exists
        if (!fs.existsSync(serverModule)) {
            throw new Error(`LSP server binary not found at: ${serverModule}`);
        }

        console.log(`LSP server path: ${serverModule}`);

        const serverOptions: ServerOptions = {
            run: { 
                command: serverModule,
                transport: TransportKind.stdio
            },
            debug: {
                command: serverModule,
                args: ['--debug'],
                transport: TransportKind.stdio,
                options: { 
                    env: { 
                        ...process.env,
                        RUST_LOG: 'debug',
                        RUST_BACKTRACE: '1'
                    } 
                }
            }
        };

        const clientOptions: LanguageClientOptions = {
            documentSelector: [
                { scheme: 'file', pattern: '**/BUILD{,.bazel}' },
                { scheme: 'file', pattern: '**/*.{bazel,bzl}' },
                { scheme: 'file', pattern: '**/WORKSPACE{,.bazel}' },
                { scheme: 'file', language: 'go' },
                { scheme: 'file', language: 'typescript' },
                { scheme: 'file', language: 'javascript' },
                { scheme: 'file', language: 'python' },
                { scheme: 'file', language: 'java' }
            ],
            synchronize: {
                fileEvents: [
                    vscode.workspace.createFileSystemWatcher('**/BUILD{,.bazel}'),
                    vscode.workspace.createFileSystemWatcher('**/*.{bazel,bzl}'),
                    vscode.workspace.createFileSystemWatcher('**/WORKSPACE{,.bazel}')
                ]
            },
            outputChannelName: 'Bazel Language Server',
            traceOutputChannel: vscode.window.createOutputChannel('Bazel LSP Trace'),
            revealOutputChannelOn: 1, // RevealOutputChannelOn.Error
            initializationOptions: {
                // Tell the server not to register these commands as they're already registered client-side
                excludeCommands: [
                    'bazel.build',
                    'bazel.test', 
                    'bazel.run',
                    'bazel.clean',
                    'bazel.showDependencies',
                    'bazel.refresh',
                    'bazel.debug',
                    'bazel.openTarget'
                ]
            }
        };

        // Create the language client and start it
        client = new LanguageClient(
            'bazel-lsp',
            'Bazel Language Server',
            serverOptions,
            clientOptions
        );

        // Register commands before starting the client
        registerCommands(context, client);

        // Register tree data provider
        const targetProvider = new BazelTargetProvider(client);
        vscode.window.createTreeView('bazelTargets', {
            treeDataProvider: targetProvider,
            showCollapseAll: true
        });

        // Start the client. This will also launch the server
        await client.start();

        // Register CodeLens provider after client has started
        const codeLensEnabled = vscode.workspace.getConfiguration('bazel').get<boolean>('enableCodeLens', true);
        if (codeLensEnabled) {
            context.subscriptions.push(
                vscode.languages.registerCodeLensProvider(
                    { pattern: '**/*.{go,ts,js,py,java}' },
                    new BazelCodeLensProvider(client)
                )
            );
        }

        // Send configuration to server after it's ready
        await sendConfiguration();

        // Listen for configuration changes
        context.subscriptions.push(
            vscode.workspace.onDidChangeConfiguration(async (e) => {
                if (e.affectsConfiguration('bazel')) {
                    await sendConfiguration();
                }
            })
        );

        console.log('Bazel language server started successfully');
        vscode.window.showInformationMessage('Bazel extension activated successfully!');

    } catch (error) {
        console.error('Failed to activate Bazel extension:', error);
        vscode.window.showErrorMessage(`Failed to activate Bazel extension: ${error}`);
        throw error;
    }
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

async function sendConfiguration() {
    if (!client) return;

    const config = vscode.workspace.getConfiguration('bazel');
    await client.sendNotification('workspace/didChangeConfiguration', {
        settings: {
            bazel: {
                executable: config.get('executable'),
                workspaceRoot: config.get('workspaceRoot'),
                buildFlags: config.get('buildFlags'),
                testFlags: config.get('testFlags'),
                cache: {
                    queryResults: config.get('cache.queryResults'),
                    ttl: config.get('cache.ttl')
                },
                parallelism: config.get('parallelism'),
                languages: {
                    go: {
                        enabled: config.get('languages.go.enabled'),
                        goplsPath: config.get('languages.go.goplsPath')
                    },
                    typescript: {
                        enabled: config.get('languages.typescript.enabled'),
                        tsserverPath: config.get('languages.typescript.tsserverPath')
                    },
                    python: {
                        enabled: config.get('languages.python.enabled'),
                        interpreter: config.get('languages.python.interpreter')
                    },
                    java: {
                        enabled: config.get('languages.java.enabled'),
                        jdtlsPath: config.get('languages.java.jdtlsPath')
                    }
                }
            }
        }
    });
}

class BazelCodeLensProvider implements vscode.CodeLensProvider {
    constructor(private client: LanguageClient) {}

    async provideCodeLenses(
        document: vscode.TextDocument,
        token: vscode.CancellationToken
    ): Promise<vscode.CodeLens[]> {
        // Check if client is ready
        if (!this.client || this.client.state !== 2) { // 2 = Running
            return [];
        }

        try {
            // Check for cancellation before making the request
            if (token.isCancellationRequested) {
                return [];
            }

            const result = await this.client.sendRequest(
                'textDocument/codeLens',
                {
                    textDocument: { uri: document.uri.toString() }
                },
                token
            );
            
            // Check for cancellation after receiving the result
            if (token.isCancellationRequested) {
                return [];
            }
            
            return result as vscode.CodeLens[];
        } catch (error: any) {
            // Handle cancellation silently
            if (error?.code === -32800 || error?.message === 'Canceled') {
                return [];
            }
            
            // Only log non-cancellation errors
            console.error('Error providing code lenses:', error);
            return [];
        }
    }
}