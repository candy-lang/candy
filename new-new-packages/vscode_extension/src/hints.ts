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
  private editors = new Map<String, vs.TextEditor>();
  private updateTimeout?: NodeJS.Timer;

  private readonly decorationTypes = new Map<
    HintKind,
    vs.TextEditorDecorationType
  >([
    [
      'value',
      vs.window.createTextEditorDecorationType({
        after: { color: new vs.ThemeColor('candy.hints.valueColor') },
        rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
      }),
    ],
    [
      'panic',
      vs.window.createTextEditorDecorationType({
        after: { color: new vs.ThemeColor('candy.hints.panicColor') },
        rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
      }),
    ],
  ]);

  constructor(private readonly analyzer: LanguageClient) {
    // tslint:disable-next-line: no-floating-promises
    analyzer.onReady().then(() => {
      this.analyzer.onNotification(
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
            // Delay this so if we're getting lots of updates, we don't flicker.
            if (this.updateTimeout) {
              clearTimeout(this.updateTimeout);
            }
            this.updateTimeout = setTimeout(() => this.update(), 500);
          }
        }
      );
    });

    this.subscriptions.push(
      vs.window.onDidChangeActiveTextEditor(() => this.update())
    );
    this.subscriptions.push(
      vs.workspace.onDidCloseTextDocument((document) => {
        this.hints.delete(document.uri.toString());
      })
    );
    if (vs.window.activeTextEditor) {
      this.update();
    }
  }

  private update() {
    const editor = vs.window.activeTextEditor;
    if (!editor || !editor.document) {
      return;
    }

    const hints = this.hints.get(editor.document.uri.toString());
    if (hints === undefined) {
      return;
    }

    type Item = vs.DecorationOptions & {
      renderOptions: { after: { contentText: string } };
    };
    const decorations = new Map<HintKind, Item[]>();
    for (const hint of hints) {
      const position = this.analyzer.protocol2CodeConverter.asPosition(
        hint.position
      );

      // Ensure the hint we got looks like a sensible position, otherwise the type info
      // might be stale (e.g., we sent two updates, and the type from in between them just
      // arrived). In this case, we'll just bail and do nothing, assuming a future update will
      // have the correct info.
      // TODO(later, JonasWanke): do we really need this check?
      if (position.character < 1) {
        return;
      }

      const existing = decorations.get(hint.kind) || [];
      existing.push({
        range: new vs.Range(position, position),
        renderOptions: { after: { contentText: hint.text } },
      });
      decorations.set(hint.kind, existing);
    }

    this.editors.set(editor.document.uri.toString(), editor);
    for (const entry of this.decorationTypes.entries()) {
      editor.setDecorations(entry[1], decorations.get(entry[0]) || []);
    }
  }

  public dispose() {
    for (const editor of this.editors.values()) {
      try {
        for (const decorationType of this.decorationTypes.values()) {
          editor.setDecorations(decorationType, []);
        }
      } catch {
        // It's possible the editor was closed, but there
        // doesn't seem to be a way to tell.
      }
    }
    this.subscriptions.forEach((s) => s.dispose());
  }
}
