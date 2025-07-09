import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';

export function registerCommands(context: vscode.ExtensionContext, client: LanguageClient) {
    // Build command
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.build', async () => {
            const target = await getTargetForCurrentFile(client);
            if (!target) {
                vscode.window.showErrorMessage('No Bazel target found for current file');
                return;
            }

            const terminal = vscode.window.createTerminal('Bazel Build');
            terminal.show();
            
            const config = vscode.workspace.getConfiguration('bazel');
            const bazelPath = config.get<string>('executable', 'bazel');
            const buildFlags = config.get<string[]>('buildFlags', []);
            
            terminal.sendText(`${bazelPath} build ${target} ${buildFlags.join(' ')}`);
        })
    );

    // Test command
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.test', async () => {
            const target = await getTargetForCurrentFile(client);
            if (!target) {
                vscode.window.showErrorMessage('No Bazel test target found for current file');
                return;
            }

            const terminal = vscode.window.createTerminal('Bazel Test');
            terminal.show();
            
            const config = vscode.workspace.getConfiguration('bazel');
            const bazelPath = config.get<string>('executable', 'bazel');
            const testFlags = config.get<string[]>('testFlags', ['--test_output=errors']);
            
            terminal.sendText(`${bazelPath} test ${target} ${testFlags.join(' ')}`);
        })
    );

    // Run command
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.run', async () => {
            const target = await getTargetForCurrentFile(client);
            if (!target) {
                vscode.window.showErrorMessage('No runnable Bazel target found for current file');
                return;
            }

            const terminal = vscode.window.createTerminal('Bazel Run');
            terminal.show();
            
            const config = vscode.workspace.getConfiguration('bazel');
            const bazelPath = config.get<string>('executable', 'bazel');
            
            terminal.sendText(`${bazelPath} run ${target}`);
        })
    );

    // Clean command
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.clean', async () => {
            const terminal = vscode.window.createTerminal('Bazel Clean');
            terminal.show();
            
            const config = vscode.workspace.getConfiguration('bazel');
            const bazelPath = config.get<string>('executable', 'bazel');
            
            terminal.sendText(`${bazelPath} clean`);
        })
    );

    // Show dependencies command
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.showDependencies', async () => {
            const target = await getTargetForCurrentFile(client);
            if (!target) {
                vscode.window.showErrorMessage('No Bazel target found for current file');
                return;
            }

            const result = await client.sendRequest<string[]>('bazel/getDependencies', { target });
            
            const panel = vscode.window.createWebviewPanel(
                'bazelDependencies',
                `Dependencies of ${target}`,
                vscode.ViewColumn.One,
                {}
            );

            panel.webview.html = generateDependencyHtml(target, result);
        })
    );

    // Refresh workspace command
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.refresh', async () => {
            await client.sendNotification('bazel/refreshWorkspace');
            vscode.window.showInformationMessage('Bazel workspace refreshed');
        })
    );

    // Open target command (for tree view clicks)
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.openTarget', async (targetLabel: string) => {
            // Query the target location from the server
            const location = await client.sendRequest<{ uri: string; range?: vscode.Range }>('bazel/getTargetLocation', { target: targetLabel });
            if (location && location.uri) {
                const uri = vscode.Uri.parse(location.uri);
                const document = await vscode.workspace.openTextDocument(uri);
                await vscode.window.showTextDocument(document);
            }
        })
    );

    // Debug command
    context.subscriptions.push(
        vscode.commands.registerCommand('bazel.debug', async () => {
            const target = await getTargetForCurrentFile(client);
            if (!target) {
                vscode.window.showErrorMessage('No debuggable Bazel target found for current file');
                return;
            }

            const editor = vscode.window.activeTextEditor;
            if (!editor) return;

            const language = editor.document.languageId;
            let debugType = '';
            
            switch (language) {
                case 'go':
                    debugType = 'bazel-go';
                    break;
                case 'python':
                    debugType = 'bazel-python';
                    break;
                default:
                    vscode.window.showErrorMessage(`Debugging not supported for ${language}. Only Go and Python are currently supported.`);
                    return;
            }

            const debugConfig: vscode.DebugConfiguration = {
                type: debugType,
                request: 'launch',
                name: `Debug ${target}`,
                target: target
            };

            await vscode.debug.startDebugging(undefined, debugConfig);
        })
    );
}

async function getTargetForCurrentFile(client: LanguageClient): Promise<string | undefined> {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return undefined;

    const result = await client.sendRequest<{ target: string }>('bazel/getTargetForFile', {
        uri: editor.document.uri.toString()
    });

    return result?.target;
}

function generateDependencyHtml(target: string, dependencies: string[]): string {
    return `
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Dependencies</title>
            <style>
                body {
                    font-family: var(--vscode-font-family);
                    color: var(--vscode-foreground);
                    background-color: var(--vscode-editor-background);
                    padding: 20px;
                }
                h1 {
                    color: var(--vscode-titleBar-activeForeground);
                }
                ul {
                    list-style-type: none;
                    padding-left: 20px;
                }
                li {
                    padding: 5px 0;
                    cursor: pointer;
                }
                li:hover {
                    color: var(--vscode-textLink-foreground);
                    text-decoration: underline;
                }
                .target {
                    font-weight: bold;
                    color: var(--vscode-terminal-ansiGreen);
                }
            </style>
        </head>
        <body>
            <h1>Dependencies of <span class="target">${target}</span></h1>
            <ul>
                ${dependencies.map(dep => `<li>${dep}</li>`).join('')}
            </ul>
        </body>
        </html>
    `;
}