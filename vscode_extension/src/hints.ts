// Lots of this code are taken from Dart-Code:
// https://github.com/Dart-Code/Dart-Code/blob/075f71ca0336e94ebb480be35895b5b12314223b/src/extension/lsp/closing_labels_decorations.ts
import * as vs from "vscode";
import { LanguageClient } from "vscode-languageclient/node";
import { Hint, HintKind, publishHintsType } from "./lsp_custom_protocol";

export class HintsDecorations implements vs.Disposable {
  private subscriptions: vs.Disposable[] = [];
  private hints = new Map<string, Hint[]>();

  private decorationTypes = new Map<HintKind, vs.TextEditorDecorationType>();

  constructor(private readonly client: LanguageClient) {
    [
      { kind: "value", color: "candy.valueHint" },
      { kind: "fuzzingStatus", color: "candy.statusHint" },
      {
        kind: "sampleInputReturningNormally",
        color: "candy.sampleInput.returningNormally",
      },
      {
        kind: "sampleInputPanickingWithCallerResponsible",
        color: "candy.sampleInput.panickingWithCallerResponsible",
      },
      {
        kind: "sampleInputPanickingWithInternalCodeResponsible",
        color: "candy.sampleInput.panickingWithInternalCodeResponsible",
      },
    ].forEach((value) =>
      this.decorationTypes.set(
        value.kind as HintKind,
        vs.window.createTextEditorDecorationType({
          after: {
            color: new vs.ThemeColor(`${value.color}.foreground`),
            backgroundColor: new vs.ThemeColor(`${value.color}.background`),
            margin: "0 0 0 16px",
          },
          rangeBehavior: vs.DecorationRangeBehavior.ClosedOpen,
        }),
      ),
    );

    this.client.onNotification(publishHintsType, (notification) => {
      // We parse the URI so that it gets normalized.
      const uri = vs.Uri.parse(notification.uri).toString();

      this.hints.set(uri, notification.hints);
      // Fire an update if it was for the active document.
      if (
        vs.window.activeTextEditor?.document &&
        uri === vs.window.activeTextEditor.document.uri.toString()
      ) {
        this.update();
      }
    });

    this.subscriptions.push(
      vs.window.onDidChangeVisibleTextEditors(() => {
        this.update();
      }),
    );
    this.subscriptions.push(
      vs.workspace.onDidCloseTextDocument((document) => {
        this.hints.delete(document.uri.toString());
      }),
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
        const position = this.client.protocol2CodeConverter.asPosition(
          hint.position,
        );
        // Ensure that the hint we got has a sensible position. Otherwise, the
        // hint might be stale (e.g., we sent two updates, and the hint from in
        // between them just arrived). In this case, we'll just bail and do
        // nothing, assuming a future update will have the correct hint.
        // TODO(later, JonasWanke): do we really need this check?
        if (position.character < 1) {
          return;
        }

        const existing = decorations.get(hint.kind) ?? [];
        existing.push({
          range: new vs.Range(position, position),
          renderOptions: { after: { contentText: hint.text } },
        });
        decorations.set(hint.kind, existing);
      }
      for (const [hintKind, decorationType] of this.decorationTypes.entries()) {
        editor.setDecorations(decorationType, decorations.get(hintKind) ?? []);
      }
    }
  }

  public dispose() {
    for (const editor of vs.window.visibleTextEditors) {
      for (const decorationType of this.decorationTypes.values()) {
        editor.setDecorations(decorationType, []);
      }
    }
    for (const subscription of this.subscriptions) {
      subscription.dispose();
    }
  }
}
