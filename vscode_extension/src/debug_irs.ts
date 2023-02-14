import { assert } from 'console';
import * as vscode from 'vscode';
import { DocumentUri, LanguageClient } from 'vscode-languageclient/node';
import {
  Ir,
  updateIrNotification,
  viewIr,
  ViewIrParams,
} from './lsp_custom_protocol';

export function registerDebugIrCommands(client: LanguageClient) {
  const updateIrEmitter = new vscode.EventEmitter<vscode.Uri>();
  const onIrUpdate = updateIrEmitter.event;
  client.onNotification(updateIrNotification, (notification) => {
    console.log('updateIrNotification', notification);
    updateIrEmitter.fire(vscode.Uri.parse(notification.uri));
  });

  registerDebugIrCommand(client, onIrUpdate, 'rcst', 'RCST', 'viewRcst');
  registerDebugIrCommand(client, onIrUpdate, 'ast', 'AST', 'viewAst');
  registerDebugIrCommand(client, onIrUpdate, 'hir', 'HIR', 'viewHir');
}

function registerDebugIrCommand(
  client: LanguageClient,
  onIrUpdate: vscode.Event<vscode.Uri>,
  ir: Ir,
  irName: string,
  command: string
) {
  const uriScheme = schemeForIr(ir);

  const emitter = new vscode.EventEmitter<vscode.Uri>();
  onIrUpdate((update) => {
    if (update.scheme != uriScheme) return;
    emitter.fire(update);
  });
  const provider = new (class implements vscode.TextDocumentContentProvider {
    onDidChange?: vscode.Event<vscode.Uri> | undefined = emitter.event;
    provideTextDocumentContent(
      uri: vscode.Uri,
      _token: vscode.CancellationToken
    ): vscode.ProviderResult<string> {
      const params: ViewIrParams = { uri: decodeUri(ir, uri), ir };
      return client.sendRequest(viewIr, params);
    }
  })();
  vscode.workspace.registerTextDocumentContentProvider(uriScheme, provider);

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

    const encodedUri = encodeUri(ir, document.uri);
    const irDocument = await vscode.workspace.openTextDocument(encodedUri);
    await vscode.window.showTextDocument(irDocument, vscode.ViewColumn.Beside);
  });
}

function encodeUri(ir: Ir, uri: vscode.Uri): vscode.Uri {
  return vscode.Uri.from({
    scheme: schemeForIr(ir),
    path: uri.path,
    query: `scheme=${encodeURIComponent(uri.scheme)}`,
  });
}
function decodeUri(ir: Ir, uri: vscode.Uri): DocumentUri {
  assert(uri.scheme === schemeForIr(ir));

  const scheme = decodeURIComponent(uri.query.substring('scheme='.length));
  return `${scheme}://${uri.path}`;
}
function schemeForIr(ir: Ir): string {
  return `candy-${ir}`;
}
