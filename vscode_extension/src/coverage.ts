// Lots of this code are taken from Dart-Code:
// https://github.com/Dart-Code/Dart-Code/blob/075f71ca0336e94ebb480be35895b5b12314223b/src/extension/lsp/closing_labels_decorations.ts
import * as vs from 'vscode';
import { LanguageClient, Range } from 'vscode-languageclient/node';
import { publishCoverageType } from './lsp_custom_protocol';

export class CoverageService implements vs.Disposable {
  private subscriptions: vs.Disposable[] = [];
  private coverages = new Map<String, Range[]>();

  private decorationType = vs.window.createTextEditorDecorationType({
    rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
    backgroundColor: new vs.ThemeColor('candy.coverage'),
    borderWidth: '1px',
    borderColor: 'black',
  });

  constructor(private readonly client: LanguageClient) {
    this.client.onNotification(publishCoverageType, (notification) => {
      // We parse the URI so that it gets normalized.
      const uri = vs.Uri.parse(notification.uri).toString();

      this.coverages.set(uri, notification.ranges);
      // Fire an update if it was for the active document.
      if (
        vs.window.activeTextEditor &&
        vs.window.activeTextEditor.document &&
        uri === vs.window.activeTextEditor.document.uri.toString()
      ) {
        this.update();
      }
    });

    this.subscriptions.push(
      vs.window.onDidChangeVisibleTextEditors(() => this.update())
    );
    this.subscriptions.push(
      vs.workspace.onDidCloseTextDocument((document) => {
        this.coverages.delete(document.uri.toString());
      })
    );
    this.update();
  }

  private update() {
    for (const editor of vs.window.visibleTextEditors) {
      const uri = editor.document.uri.toString();
      const ranges = this.coverages.get(uri);
      if (ranges === undefined) {
        return;
      }
      let decorations = [];
      for (const range of ranges) {
        decorations.push({
          range: new vs.Range(
            this.client.protocol2CodeConverter.asPosition(range.start),
            this.client.protocol2CodeConverter.asPosition(range.end)
          ),
        });
      }
      editor.setDecorations(this.decorationType, decorations);
    }
  }

  public dispose() {
    for (const editor of vs.window.visibleTextEditors) {
      editor.setDecorations(this.decorationType, []);
    }
    this.subscriptions.forEach((s) => s.dispose());
  }
}
