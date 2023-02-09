import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { viewRcst } from './lsp_custom_protocol';

export function registerDebugIrCommands(client: LanguageClient) {
  const provider = new (class implements vscode.TextDocumentContentProvider {
    onDidChange?: vscode.Event<vscode.Uri> | undefined;
    provideTextDocumentContent(
      uri: vscode.Uri,
      token: vscode.CancellationToken
    ): vscode.ProviderResult<string> {
      if (uri.scheme !== 'candy-rcst') return null;

      const scheme = decodeURIComponent(uri.query.substring('scheme='.length));
      const originalUri = `${scheme}://${uri.path}`;
      console.log('Requesting RCST for URI', originalUri);
      const params = { uri: originalUri };
      return client.sendRequest(viewRcst, params);
    }
  })();
  vscode.workspace.registerTextDocumentContentProvider('candy-rcst', provider);
  vscode.commands.registerCommand('candy.debug.viewRcst', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showErrorMessage(
        "Can't show the RCST without an active editor."
      );
      return;
    }
    if (editor.document.languageId !== 'candy') {
      vscode.window.showErrorMessage(
        "Can't show the RCST for a non-Candy file."
      );
      return;
    }

    const uri = vscode.Uri.from({
      scheme: 'candy-rcst',
      path: editor.document.uri.path,
      query: `scheme=${encodeURIComponent(editor.document.uri.scheme)}`,
    });
    const document = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(document, vscode.ViewColumn.Beside);
  });
}
