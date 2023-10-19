import * as vs from "vscode";
import { LanguageClient } from "vscode-languageclient/node";
import { publishServerStatusType } from "./lsp_custom_protocol";

export class ServerStatusService implements vs.Disposable {
  private subscriptions: vs.Disposable[] = [];
  private item: vs.StatusBarItem;

  constructor(private readonly client: LanguageClient) {
    this.item = vs.window.createStatusBarItem(vs.StatusBarAlignment.Left);
    this.item.text = "🍭 Starting";
    this.item.tooltip = "Candy server";
    this.item.show();
    console.log("Status bar item shown");

    this.subscriptions.push(
      client.onNotification(
        publishServerStatusType,
        (notification) => (this.item.text = notification.text),
      ),
    );
  }

  public dispose() {
    for (const subscription of this.subscriptions) {
      subscription.dispose();
    }
    this.item.dispose();
    this.item.hide();
  }
}
