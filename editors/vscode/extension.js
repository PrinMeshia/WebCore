const vscode = require('vscode');
const cp = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

/**
 * Format a .webc document by running `webc fmt` on a temporary copy.
 *
 * The process runs with the document's workspace folder as cwd so that a
 * project-level `webc.toml` (`[fmt] indent = N`) is honoured; the
 * `webcore.formatIndent` setting, when set, overrides it via `--indent`.
 */
function formatDocument(document) {
  const config = vscode.workspace.getConfiguration('webcore', document.uri);
  const bin = config.get('formatterPath') || 'webc';
  const indent = config.get('formatIndent');

  const tmp = path.join(
    os.tmpdir(),
    `webc-fmt-${process.pid}-${Date.now()}.webc`
  );
  fs.writeFileSync(tmp, document.getText(), 'utf8');

  const args = ['fmt'];
  if (Number.isInteger(indent) && indent > 0) {
    args.push('--indent', String(indent));
  }
  args.push(tmp);

  const folder = vscode.workspace.getWorkspaceFolder(document.uri);
  try {
    cp.execFileSync(bin, args, {
      cwd: folder ? folder.uri.fsPath : undefined,
      timeout: 10000,
    });
    const formatted = fs.readFileSync(tmp, 'utf8');
    if (formatted === document.getText()) {
      return [];
    }
    const fullRange = new vscode.Range(
      document.positionAt(0),
      document.positionAt(document.getText().length)
    );
    return [vscode.TextEdit.replace(fullRange, formatted)];
  } catch (err) {
    if (err && err.code === 'ENOENT') {
      vscode.window.showWarningMessage(
        `WebCore : binaire « ${bin} » introuvable. Installez webc ou réglez webcore.formatterPath.`
      );
    } else {
      const msg = (err.stderr ? String(err.stderr) : String(err.message || err))
        .trim()
        .split('\n')[0];
      vscode.window.showWarningMessage(`webc fmt : ${msg}`);
    }
    return [];
  } finally {
    fs.rmSync(tmp, { force: true });
  }
}

/**
 * Run `webc check --json` at the project root and publish the diagnostics.
 *
 * The report is a single JSON line: positioned diagnostics (parse errors)
 * land on their file; project-level issues (unknown component, bad route…)
 * are attached to webc.toml.
 */
function refreshDiagnostics(document, collection) {
  const folder = vscode.workspace.getWorkspaceFolder(document.uri);
  if (!folder) {
    return;
  }
  const root = folder.uri.fsPath;
  if (!fs.existsSync(path.join(root, 'webc.toml'))) {
    return;
  }
  const config = vscode.workspace.getConfiguration('webcore', document.uri);
  const bin = config.get('formatterPath') || 'webc';

  cp.execFile(
    bin,
    ['check', '--json'],
    { cwd: root, timeout: 15000 },
    (_err, stdout) => {
      let report;
      try {
        report = JSON.parse(String(stdout).trim());
      } catch {
        return; // old binary without --json, or crash: stay silent
      }
      collection.clear();
      const byFile = new Map();
      for (const d of report.diagnostics || []) {
        const file = d.file || 'webc.toml';
        const fsPath = path.isAbsolute(file) ? file : path.join(root, file);
        const line = Math.max(0, (d.line || 1) - 1);
        const col = Math.max(0, (d.col || 1) - 1);
        const diag = new vscode.Diagnostic(
          new vscode.Range(line, col, line, col + 1),
          d.message,
          d.severity === 'warning'
            ? vscode.DiagnosticSeverity.Warning
            : vscode.DiagnosticSeverity.Error
        );
        diag.source = 'webc';
        diag.code = d.code;
        if (!byFile.has(fsPath)) {
          byFile.set(fsPath, []);
        }
        byFile.get(fsPath).push(diag);
      }
      for (const [fsPath, diags] of byFile) {
        collection.set(vscode.Uri.file(fsPath), diags);
      }
    }
  );
}

function activate(context) {
  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider('webc', {
      provideDocumentFormattingEdits(document) {
        return formatDocument(document);
      },
    })
  );

  const collection = vscode.languages.createDiagnosticCollection('webc');
  context.subscriptions.push(collection);
  const check = (document) => {
    if (document.languageId === 'webc') {
      refreshDiagnostics(document, collection);
    }
  };
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument(check),
    vscode.workspace.onDidOpenTextDocument(check)
  );
  if (vscode.window.activeTextEditor) {
    check(vscode.window.activeTextEditor.document);
  }
}

function deactivate() {}

module.exports = { activate, deactivate };
