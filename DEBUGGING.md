# Debugging VSCode Bazel Extension Activation

## Quick Test

1. **Open VSCode Developer Console**:
   - Press `Cmd+Shift+P` → "Developer: Toggle Developer Tools"
   - Go to the Console tab
   - Look for any red errors

2. **Check Extension Host Log**:
   - Press `Cmd+Shift+P` → "Developer: Show Logs..."
   - Select "Extension Host"
   - Look for "Bazel" related messages

## Step-by-Step Debugging

### 1. Verify Extension Installation

```bash
# List installed extensions
code --list-extensions | grep bazel

# If not found, install it:
code --install-extension ./extension/bazel-extension-0.1.0.vsix
```

### 2. Force Reload VSCode

After installing:
- Press `Cmd+Shift+P` → "Developer: Reload Window"

### 3. Check Activation Triggers

The extension should activate when:
- Opening a file named `BUILD` or `BUILD.bazel`
- Opening a file named `WORKSPACE` or `WORKSPACE.bazel`
- Opening a `.bazel` or `.bzl` file
- Running any Bazel command (e.g., `Cmd+Shift+P` → "Bazel: Build Target")

### 4. Test with Simple Activation

Try opening the test BUILD file:
```bash
code test/BUILD
```

### 5. Manual Activation Test

1. Open any file in VSCode
2. Press `Cmd+Shift+P`
3. Type "Bazel: Build Target"
4. Press Enter
5. This should force activation

### 6. Check Output Channels

1. Go to View → Output
2. From the dropdown, select:
   - "Bazel Language Server" - for server logs
   - "Bazel LSP Trace" - for LSP communication
   - "Extension Host" - for general extension logs

### 7. Common Issues and Solutions

#### Extension Not Listed
```bash
# Reinstall
code --uninstall-extension askscio.bazel-extension
code --install-extension ./extension/bazel-extension-0.1.0.vsix
```

#### Binary Permission Issues (macOS)
```bash
# Make LSP server executable
chmod +x extension/server/bazel-lsp

# Clear quarantine flag (macOS Gatekeeper)
xattr -d com.apple.quarantine extension/server/bazel-lsp
```

#### Path Issues
Check if the LSP server exists:
```bash
ls -la extension/server/bazel-lsp
```

### 8. Test Minimal Extension

If nothing works, test the minimal version:
```bash
# Package minimal test
cd extension
mv package.json package-full.json
mv package-test.json package.json
npx vsce package --no-dependencies
code --install-extension bazel-extension-test-0.1.0.vsix
```

This minimal extension will show:
- An info message when activated
- A status bar item saying "✓ Bazel Active"
- A test command in the command palette

### 9. Enable Trace Logging

Add to VSCode settings.json:
```json
{
  "bazel.trace.server": "verbose"
}
```

### 10. Check System Logs

On macOS:
```bash
# Check system logs for crashes
log show --predicate 'process == "bazel-lsp"' --last 5m
```

## If All Else Fails

1. **Create an Issue** with:
   - VSCode version: `code --version`
   - OS version
   - Contents of Developer Console
   - Extension Host logs
   - Output from: `ls -la extension/server/`

2. **Try Development Mode**:
   - Clone the repo
   - Open in VSCode
   - Press F5 to launch Extension Development Host
   - This will show more detailed debugging info 