import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import {
  updateIrNotification,
  viewIr,
  ViewIrParams,
} from './lsp_custom_protocol';
import { combineCancellationTokens } from './utils';

type Ir =
  | { type: 'rcst' }
  | { type: 'ast' }
  | { type: 'hir' }
  | { type: 'mir'; tracingConfig: TracingConfig }
  | { type: 'optimizedMir'; tracingConfig: TracingConfig }
  | { type: 'lir'; tracingConfig: TracingConfig };
type IrType = Ir['type'];
function getIrTitle(irType: IrType): string {
  switch (irType) {
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
    case 'lir':
      return 'LIR';
  }
}

export function registerDebugIrCommands(client: LanguageClient) {
  const updateIrEmitter = new vscode.EventEmitter<vscode.Uri>();
  registerDocumentProvider(client, updateIrEmitter.event);
  client.onNotification(updateIrNotification, (notification) => {
    updateIrEmitter.fire(vscode.Uri.parse(notification.uri));
  });

  registerDebugIrCommand('rcst', 'viewRcst', async () => ({ type: 'rcst' }));
  registerDebugIrCommand('ast', 'viewAst', async () => ({ type: 'ast' }));
  registerDebugIrCommand('hir', 'viewHir', async () => ({ type: 'hir' }));
  registerDebugIrCommand('mir', 'viewMir', async () => {
    const tracingConfig = await pickTracingConfig({
      canSelectOnlyCurrent: false,
    });
    if (tracingConfig === undefined) return undefined;

    return { type: 'mir', tracingConfig };
  });
  registerDebugIrCommand('optimizedMir', 'viewOptimizedMir', async () => {
    const tracingConfig = await pickTracingConfig();
    if (tracingConfig === undefined) return undefined;

    return { type: 'optimizedMir', tracingConfig };
  });
  registerDebugIrCommand('lir', 'viewLir', async () => {
    const tracingConfig = await pickTracingConfig();
    if (tracingConfig === undefined) return undefined;

    return { type: 'lir', tracingConfig };
  });
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
          title: `Loading ${getIrTitle(ir.type)} of ${originalUri}…`,
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
async function registerDebugIrCommand(
  irType: IrType,
  command: string,
  createIrConfig: () => Promise<Ir | undefined>
) {
  vscode.commands.registerCommand(`candy.debug.${command}`, async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showErrorMessage(
        `Can't show the ${getIrTitle(irType)} without an active editor.`
      );
      return;
    }

    const document = editor.document;
    if (document.languageId !== 'candy') {
      vscode.window.showErrorMessage(
        `Can't show the ${getIrTitle(irType)} for a non-Candy file.`
      );
      return;
    }

    const ir = await createIrConfig();
    if (ir === undefined) return;

    const encodedUri = encodeUri(document.uri, ir);
    const irDocument = await vscode.workspace.openTextDocument(encodedUri);
    await vscode.window.showTextDocument(irDocument, vscode.ViewColumn.Beside);
  });
}

// Tracing Config

interface TracingConfig {
  registerFuzzables: TracingMode;
  calls: TracingMode;
  evaluatedExpressions: TracingMode;
}
type TracingMode = 'off' | 'onlyCurrent' | 'all';

async function pickTracingConfig(
  options: { canSelectOnlyCurrent: boolean } = { canSelectOnlyCurrent: true }
): Promise<TracingConfig | undefined> {
  const registerFuzzables = await pickTracingMode(
    'Include tracing of fuzzable closures?',
    options
  );
  if (registerFuzzables === undefined) return;

  const calls = await pickTracingMode('Include tracing of calls?', options);
  if (calls === undefined) return;

  const evaluatedExpressions = await pickTracingMode(
    'Include tracing of evaluated expressions?',
    options
  );
  if (evaluatedExpressions === undefined) return;

  return { registerFuzzables, calls, evaluatedExpressions };
}
async function pickTracingMode(
  title: string,
  options: { canSelectOnlyCurrent: boolean } = { canSelectOnlyCurrent: true }
): Promise<TracingMode | undefined> {
  type Item = vscode.QuickPickItem & { mode: TracingMode };
  const items: Item[] = [{ label: 'No', mode: 'off' }];
  if (options.canSelectOnlyCurrent) {
    items.push({ label: 'Only for the current module', mode: 'onlyCurrent' });
  }
  items.push({ label: 'Yes', mode: 'all' });

  const result = await vscode.window.showQuickPick<Item>(items, { title });
  return result?.mode;
}

// URI en-/decoding

const irScheme = 'candy-ir';
function encodeUri(uri: vscode.Uri, ir: Ir): vscode.Uri {
  const details: { [key: string]: any } = { scheme: uri.scheme, ...ir };
  delete details.type;

  return vscode.Uri.from({
    scheme: irScheme,
    path: `${uri.path}.${ir.type}`,
    // TODO: Encode this in the query part once VS Code doesn't encode it again.
    fragment: JSON.stringify(details),
  });
}
function decodeUri(uri: vscode.Uri): { ir: Ir; originalUri: vscode.Uri } {
  const details = JSON.parse(uri.fragment);
  const scheme = details.scheme as string;
  delete details.scheme;

  const separatorIndex = uri.path.lastIndexOf('.');
  const path = uri.path.slice(0, separatorIndex);

  const ir = {
    type: uri.path.slice(separatorIndex + 1) as IrType,
    ...details,
  } as Ir;
  return {
    ir,
    originalUri: vscode.Uri.from({ scheme, path }),
  };
}
