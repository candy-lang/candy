// Lots of this code are taken from Dart-Code:
// https://github.com/Dart-Code/Dart-Code/blob/075f71ca0336e94ebb480be35895b5b12314223b/src/extension/lsp/closing_labels_decorations.ts
import * as vs from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { PublishServerStatusNotification } from './lsp_custom_protocol';

export class ServerStatusService implements vs.Disposable {
  private subscriptions: vs.Disposable[] = [];
  private item: vs.StatusBarItem;

  constructor(private readonly client: LanguageClient) {
    this.item = vs.window.createStatusBarItem(vs.StatusBarAlignment.Left);
    this.item.text = '🍭 Starting';
    this.item.tooltip = 'Candy server';
    this.item.show();
    console.log('Status bar item shown');

    this.subscriptions.push(
      client.onNotification(
        PublishServerStatusNotification.type,
        (notification) => (this.item.text = notification.text)
      )
    );
  }

  public dispose() {
    this.item.hide();
  }
}
