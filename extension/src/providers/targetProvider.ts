import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';

interface BazelTarget {
    label: string;
    kind: string;
    package: string;
    srcs?: string[];
    deps?: string[];
}

export class BazelTargetProvider implements vscode.TreeDataProvider<BazelTargetItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<BazelTargetItem | undefined | null | void> = new vscode.EventEmitter<BazelTargetItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<BazelTargetItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private targets: Map<string, BazelTarget[]> = new Map();

    constructor(private client: LanguageClient) {
        this.refresh();
        
        // Listen for workspace changes
        client.onNotification('bazel/targetsChanged', () => {
            this.refresh();
        });
    }

    refresh(): void {
        this.loadTargets().then(() => {
            this._onDidChangeTreeData.fire();
        });
    }

    getTreeItem(element: BazelTargetItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: BazelTargetItem): Promise<BazelTargetItem[]> {
        if (!element) {
            // Root level - show packages
            const packages = Array.from(this.targets.keys()).sort();
            return packages.map(pkg => new BazelTargetItem(
                pkg,
                'package',
                vscode.TreeItemCollapsibleState.Collapsed
            ));
        } else if (element.type === 'package') {
            // Package level - show targets
            const targets = this.targets.get(element.label) || [];
            return targets.map(target => new BazelTargetItem(
                target.label.split(':')[1], // Just the target name
                'target',
                vscode.TreeItemCollapsibleState.None,
                target
            ));
        }
        
        return [];
    }

    private async loadTargets(): Promise<void> {
        try {
            const result = await this.client.sendRequest<BazelTarget[]>('bazel/getAllTargets');
            
            // Group targets by package
            this.targets.clear();
            for (const target of result) {
                if (!this.targets.has(target.package)) {
                    this.targets.set(target.package, []);
                }
                this.targets.get(target.package)!.push(target);
            }
        } catch (error) {
            console.error('Failed to load Bazel targets:', error);
        }
    }
}

class BazelTargetItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly type: 'package' | 'target',
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly target?: BazelTarget
    ) {
        super(label, collapsibleState);

        this.tooltip = this.makeTooltip();
        this.contextValue = type;

        if (type === 'target' && target) {
            this.iconPath = this.getIcon(target.kind);
            
            // Add click command
            this.command = {
                command: 'bazel.openTarget',
                title: 'Open Target',
                arguments: [target.label]
            };
        }
    }

    private makeTooltip(): string {
        if (this.type === 'package') {
            return `Package: ${this.label}`;
        } else if (this.target) {
            let tooltip = `${this.target.label}\nKind: ${this.target.kind}`;
            if (this.target.srcs && this.target.srcs.length > 0) {
                tooltip += `\nSources: ${this.target.srcs.length} file(s)`;
            }
            if (this.target.deps && this.target.deps.length > 0) {
                tooltip += `\nDependencies: ${this.target.deps.length}`;
            }
            return tooltip;
        }
        return this.label;
    }

    private getIcon(kind: string): vscode.ThemeIcon {
        // Map Bazel rule kinds to VSCode icons
        switch (kind) {
            case 'go_binary':
            case 'go_library':
            case 'go_test':
                return new vscode.ThemeIcon('symbol-method');
            case 'ts_library':
            case 'ts_project':
                return new vscode.ThemeIcon('symbol-class');
            case 'py_binary':
            case 'py_library':
            case 'py_test':
                return new vscode.ThemeIcon('symbol-namespace');
            case 'java_binary':
            case 'java_library':
            case 'java_test':
                return new vscode.ThemeIcon('symbol-interface');
            case 'proto_library':
                return new vscode.ThemeIcon('symbol-struct');
            case 'filegroup':
                return new vscode.ThemeIcon('files');
            default:
                return new vscode.ThemeIcon('symbol-misc');
        }
    }
}