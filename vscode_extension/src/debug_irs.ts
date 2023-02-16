import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import {
  updateIrNotification,
  viewIr,
  ViewIrParams,
} from './lsp_custom_protocol';

export function registerDebugIrCommands(client: LanguageClient) {
  const updateIrEmitter = new vscode.EventEmitter<vscode.Uri>();
  registerDocumentProvider(client, updateIrEmitter.event);
  client.onNotification(updateIrNotification, (notification) => {
    console.log('updateIrNotification', notification);
    updateIrEmitter.fire(vscode.Uri.parse(notification.uri));
  });

  registerDebugIrCommand('rcst', 'RCST', 'viewRcst');
  registerDebugIrCommand('ast', 'AST', 'viewAst');
  registerDebugIrCommand('hir', 'HIR', 'viewHir');
}

function registerDocumentProvider(
  client: LanguageClient,
  onIrUpdate: vscode.Event<vscode.Uri>
) {
  const provider = new (class implements vscode.TextDocumentContentProvider {
    onDidChange?: vscode.Event<vscode.Uri> | undefined = onIrUpdate;
    provideTextDocumentContent(
      uri: vscode.Uri,
      _token: vscode.CancellationToken
    ): vscode.ProviderResult<string> {
      const params: ViewIrParams = { uri: uri.toString() };
      return client.sendRequest(viewIr, params);
    }
  })();
  vscode.workspace.registerTextDocumentContentProvider(irScheme, provider);
}
function registerDebugIrCommand(ir: string, irName: string, command: string) {
  vscode.commands.registerCommand(`candy.debug.${command}`, async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showErrorMessage(
        `Can't show the ${irName} without an active editor.`
      );
      return;
    }
    const document = editor.document;
    if (document.languageId !== 'candy') {
      vscode.window.showErrorMessage(
        `Can't show the ${irName} for a non-Candy file.`
      );
      return;
    }

    const encodedUri = encodeUri(document.uri, ir);
    console.log('encodedUri', encodedUri, encodedUri.toString());
    const irDocument = await vscode.workspace.openTextDocument(encodedUri);
    await vscode.window.showTextDocument(irDocument, vscode.ViewColumn.Beside);
  });
}

const irScheme = 'candy-ir';
function encodeUri(uri: vscode.Uri, ir: string): vscode.Uri {
  return vscode.Uri.from({
    scheme: irScheme,
    path: `${uri.path}.${ir}`,
    // TODO: Encode this in the query part once VS Code doesn't encode it again.
    fragment: uri.scheme,
  });
}
