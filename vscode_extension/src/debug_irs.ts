import * as vscode from 'vscode';
import { LanguageClient, RequestType } from 'vscode-languageclient/node';
import { viewAst, ViewIrParams, viewRcst } from './lsp_custom_protocol';

export function registerDebugIrCommands(client: LanguageClient) {
  registerDebugIrCommand(
    client,
    'RCST',
    'candy-rcst',
    'candy.debug.viewRcst',
    viewRcst
  );
  registerDebugIrCommand(
    client,
    'AST',
    'candy-ast',
    'candy.debug.viewAst',
    viewAst
  );
}

function registerDebugIrCommand(
  client: LanguageClient,
  irName: string,
  uriScheme: string,
  command: string,
  lspRequest: RequestType<ViewIrParams, string, void>
) {
  const provider = new (class implements vscode.TextDocumentContentProvider {
    onDidChange?: vscode.Event<vscode.Uri> | undefined;
    provideTextDocumentContent(
      uri: vscode.Uri,
      _token: vscode.CancellationToken
    ): vscode.ProviderResult<string> {
      if (uri.scheme !== uriScheme) return null;

      const scheme = decodeURIComponent(uri.query.substring('scheme='.length));
      const params = { uri: `${scheme}://${uri.path}` };
      return client.sendRequest(lspRequest, params);
    }
  })();
  vscode.workspace.registerTextDocumentContentProvider(uriScheme, provider);
  vscode.commands.registerCommand(command, async () => {
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

    const uri = vscode.Uri.from({
      scheme: uriScheme,
      path: document.uri.path,
      query: `scheme=${encodeURIComponent(document.uri.scheme)}`,
    });
    const irDocument = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(irDocument, vscode.ViewColumn.Beside);
  });
}
