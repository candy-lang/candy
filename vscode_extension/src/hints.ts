// Lots of this code are taken from Dart-Code:
// https://github.com/Dart-Code/Dart-Code/blob/075f71ca0336e94ebb480be35895b5b12314223b/src/extension/lsp/closing_labels_decorations.ts
import * as vs from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import {
  Hint,
  HintKind,
  PublishHintsNotification,
} from './lsp_custom_protocol';

export class HintsDecorations implements vs.Disposable {
  private subscriptions: vs.Disposable[] = [];
  private hints = new Map<String, Hint[]>();

  private readonly decorationTypes = new Map<
    HintKind,
    vs.TextEditorDecorationType
  >([
    [
      'value',
      vs.window.createTextEditorDecorationType({
        after: { color: new vs.ThemeColor('candy.hints.valueColor') },
        rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
        isWholeLine: true,
      }),
    ],
    [
      'panic',
      vs.window.createTextEditorDecorationType({
        after: { color: new vs.ThemeColor('candy.hints.panicColor') },
        rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
        isWholeLine: true,
      }),
    ],
    [
      'fuzz',
      vs.window.createTextEditorDecorationType({
        after: { color: new vs.ThemeColor('candy.hints.fuzzColor') },
        rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
        isWholeLine: true,
      }),
    ],
    [
      'fuzzCallSite',
      vs.window.createTextEditorDecorationType({
        after: { color: new vs.ThemeColor('candy.hints.fuzzColor') },
        rangeBehavior: vs.DecorationRangeBehavior.ClosedClosed,
        backgroundColor: 'rgba(255, 0, 0, 0.2)',
      }),
    ],
  ]);

  constructor(private readonly client: LanguageClient) {
    this.client.onNotification(
      PublishHintsNotification.type,
      (notification) => {
        // We parse the URI so that it gets normalized.
        const uri = vs.Uri.parse(notification.uri).toString();

        this.hints.set(uri, notification.hints);
        // Fire an update if it was for the active document.
        if (
          vs.window.activeTextEditor &&
          vs.window.activeTextEditor.document &&
          uri === vs.window.activeTextEditor.document.uri.toString()
        ) {
          this.update();
        }
      }
    );

    this.subscriptions.push(
      vs.window.onDidChangeVisibleTextEditors(() => this.update())
    );
    this.subscriptions.push(
      vs.workspace.onDidCloseTextDocument((document) => {
        this.hints.delete(document.uri.toString());
      })
    );
    this.update();
  }

  private update() {
    for (const editor of vs.window.visibleTextEditors) {
      const uri = editor.document.uri.toString();
      const hints = this.hints.get(uri);
      if (hints === undefined) {
        return;
      }

      type Item = vs.DecorationOptions & {
        renderOptions: { after: { contentText: string } };
      };
      const decorations = new Map<HintKind, Item[]>();
      for (const hint of hints) {
        const range = new vs.Range(
          this.client.protocol2CodeConverter.asPosition(hint.range.start),
          this.client.protocol2CodeConverter.asPosition(hint.range.end)
        );

        const existing = decorations.get(hint.kind) || [];
        existing.push({
          range: range,
          renderOptions: { after: { contentText: hint.text } },
        });
        decorations.set(hint.kind, existing);
      }

      for (const [hintKind, decorationType] of this.decorationTypes.entries()) {
        editor.setDecorations(decorationType, decorations.get(hintKind) || []);
      }
    }
  }

  public dispose() {
    for (const editor of vs.window.visibleTextEditors) {
      for (const decorationType of this.decorationTypes.values()) {
        editor.setDecorations(decorationType, []);
      }
    }
    this.subscriptions.forEach((s) => s.dispose());
  }
}
