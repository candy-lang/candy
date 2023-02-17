import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import {
  updateIrNotification,
  viewIr,
  ViewIrParams,
} from './lsp_custom_protocol';
import { combineCancellationTokens } from './utils';

type Ir = 'rcst' | 'ast' | 'hir' | 'mir' | 'optimizedMir';
function getIrTitle(ir: Ir): string {
  switch (ir) {
    case 'rcst':
      return 'RCST';
    case 'ast':
      return 'AST';
    case 'hir':
      return 'HIR';
    case 'mir':
      return 'MIR';
    case 'optimizedMir':
      return 'Optimized MIR';
  }
}

export function registerDebugIrCommands(client: LanguageClient) {
  const updateIrEmitter = new vscode.EventEmitter<vscode.Uri>();
  registerDocumentProvider(client, updateIrEmitter.event);
  client.onNotification(updateIrNotification, (notification) => {
    updateIrEmitter.fire(vscode.Uri.parse(notification.uri));
  });

  registerDebugIrCommand('rcst', 'viewRcst');
  registerDebugIrCommand('ast', 'viewAst');
  registerDebugIrCommand('hir', 'viewHir');
  registerDebugIrCommand('mir', 'viewMir');
  registerDebugIrCommand('optimizedMir', 'viewOptimizedMir');
}

function registerDocumentProvider(
  client: LanguageClient,
  onIrUpdate: vscode.Event<vscode.Uri>
) {
  const provider = new (class implements vscode.TextDocumentContentProvider {
    onDidChange?: vscode.Event<vscode.Uri> | undefined = onIrUpdate;
    provideTextDocumentContent(
      uri: vscode.Uri,
      token: vscode.CancellationToken
    ): vscode.ProviderResult<string> {
      const params: ViewIrParams = { uri: uri.toString() };
      const { ir, originalUri } = decodeUri(uri);
      return vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: `Loading ${getIrTitle(ir)} of ${originalUri}…`,
          cancellable: true,
        },
        (_progress, progressCancellationToken) =>
          client.sendRequest(
            viewIr,
            params,
            combineCancellationTokens(token, progressCancellationToken)
          )
      );
    }
  })();
  vscode.workspace.registerTextDocumentContentProvider(irScheme, provider);
}
function registerDebugIrCommand(ir: Ir, command: string) {
  vscode.commands.registerCommand(`candy.debug.${command}`, async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showErrorMessage(
        `Can't show the ${getIrTitle(ir)} without an active editor.`
      );
    }
    const document = editor.document;
    if (document.languageId !== 'candy') {
      vscode.window.showErrorMessage(
        `Can't show the ${getIrTitle(ir)} for a non-Candy file.`
      );
      return;
    }

    const encodedUri = encodeUri(document.uri, ir);
    const irDocument = await vscode.workspace.openTextDocument(encodedUri);
    await vscode.window.showTextDocument(irDocument, vscode.ViewColumn.Beside);
  });
}

const irScheme = 'candy-ir';
function encodeUri(uri: vscode.Uri, ir: Ir): vscode.Uri {
  return vscode.Uri.from({
    scheme: irScheme,
    path: `${uri.path}.${ir}`,
    // TODO: Encode this in the query part once VS Code doesn't encode it again.
    fragment: uri.scheme,
  });
}
function decodeUri(uri: vscode.Uri): { ir: Ir; originalUri: vscode.Uri } {
  const separatorIndex = uri.path.lastIndexOf('.');
  const path = uri.path.slice(0, separatorIndex);
  const ir = uri.path.slice(separatorIndex + 1) as Ir;
  return {
    ir,
    originalUri: vscode.Uri.from({ scheme: uri.fragment, path }),
  };
}
