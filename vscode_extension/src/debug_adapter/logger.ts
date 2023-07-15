import * as vscode from "vscode";

export class CandyDebugAdapterLoggerFactory
  implements vscode.DebugAdapterTrackerFactory
{
  createDebugAdapterTracker(
    session: vscode.DebugSession,
  ): vscode.DebugAdapterTracker {
    return new CandyDebugAdapterLogger(session);
  }
}

class CandyDebugAdapterLogger implements vscode.DebugAdapterTracker {
  constructor(private readonly session: vscode.DebugSession) {}

  public onWillStartSession(): void {
    console.log(`Starting debug session ${this.session.id}`);
  }

  public onWillReceiveMessage(message: unknown): void {
    console.log(`==> ${JSON.stringify(message)}`);
  }

  public onDidSendMessage(message: unknown): void {
    console.log(`<== ${JSON.stringify(message)}`);
  }

  public onWillStopSession() {
    console.log(`Stopping debug session ${this.session.id}`);
  }

  public onError(error: Error): void {
    console.error(
      `Debug session ${this.session.id} errored: ${JSON.stringify(error)}`,
    );
  }

  public onExit(code: number | undefined, signal: string | undefined): void {
    console.log(
      `Debug session ${this.session.id} exit: code: ${code}, signal: ${signal}`,
    );
  }
}
