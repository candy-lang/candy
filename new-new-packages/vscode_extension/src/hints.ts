// Lots of this code are taken from Dart-Code:
// https://github.com/Dart-Code/Dart-Code/blob/1d86cf3a4fcb3653376092f6677447cd9870b98e/src/extension/lsp/closing_hints_decorations.ts
import * as vs from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { Hint, PublishHintsNotification } from './lsp_custom_protocol';

export class HintsDecorations implements vs.Disposable {
  private subscriptions: vs.Disposable[] = [];
  private hints: Map<String, Hint[]> = new Map();
  private editors: Map<String, vs.TextEditor> = new Map();
  private updateTimeout?: NodeJS.Timer;

  private readonly decorationType = vs.window.createTextEditorDecorationType({
    after: { color: new vs.ThemeColor('candy.hints') },
    rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
  });

  constructor(private readonly analyzer: LanguageClient) {
    // tslint:disable-next-line: no-floating-promises
    analyzer.onReady().then(() => {
      this.analyzer.onNotification(PublishHintsNotification.type, (n) => {
        console.log('Received hints!');

        // We parse the URI so that it gets normalized.
        const uri = vs.Uri.parse(n.uri).toString();

        this.hints.set(uri, n.hints);
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
      });
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
    const decorations: Item[] = [];
    for (const r of hints) {
      const hintRange = this.analyzer.protocol2CodeConverter.asRange(r.range);

      // Ensure the hint we got looks like a sensible range, otherwise the type info
      // might be stale (eg. we sent two updates, and the type from in between them just
      // arrived). In this case, we'll just bail and do nothing, assuming a future update will
      // have the correct info.
      // TODO(later, JonasWanke): do we really need this check?
      const finalCharacterPosition = hintRange.end;
      if (finalCharacterPosition.character < 1) {
        return;
      }

      decorations.push({
        range: hintRange,
        renderOptions: { after: { contentText: r.text } },
      });
    }

    this.editors.set(editor.document.uri.toString(), editor);
    editor.setDecorations(this.decorationType, decorations);
  }

  public dispose() {
    for (const editor of this.editors.values()) {
      try {
        editor.setDecorations(this.decorationType, []);
      } catch {
        // It's possible the editor was closed, but there
        // doesn't seem to be a way to tell.
      }
    }
    this.subscriptions.forEach((s) => s.dispose());
  }
}
