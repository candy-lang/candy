import * as path from 'path';
import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { CandyDebugAdapterLoggerFactory } from './debug_adapter/logger';
import {
  debugAdapterCreate,
  debugAdapterMessage,
  DebugSessionId,
} from './lsp_custom_protocol';

export function registerDebugAdapter(
  context: vscode.ExtensionContext,
  client: LanguageClient
) {
  const loggerFactory = new CandyDebugAdapterLoggerFactory();
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterTrackerFactory('candy', loggerFactory)
  );

  const descriptorFactory = new CandyDebugAdapterDescriptorFactory(client);
  context.subscriptions.push(
    descriptorFactory,
    vscode.debug.registerDebugAdapterDescriptorFactory(
      'candy',
      descriptorFactory
    )
  );
}

class CandyDebugAdapterDescriptorFactory
  implements vscode.DebugAdapterDescriptorFactory, vscode.Disposable
{
  constructor(private readonly client: LanguageClient) {}

  private readonly debugAdapters = new Map<
    DebugSessionId,
    vscode.DebugAdapter
  >();

  private readonly onNotificationDisposable = this.client.onNotification(
    debugAdapterMessage,
    (message) => {
      const debugAdapter = this.debugAdapters.get(message.debugSessionId);
      if (!debugAdapter) {
        console.error(
          `No debug adapter found with ID ${message.debugSessionId}`
        );
        return;
      }

      debugAdapter.handleMessage(message);
    }
  );

  async createDebugAdapterDescriptor(
    session: vscode.DebugSession,
    _executable: vscode.DebugAdapterExecutable | undefined
  ): Promise<vscode.DebugAdapterDescriptor | null | undefined> {
    const program = this.resolveProgram(
      session.configuration.program,
      session.workspaceFolder
    );
    if (!program) {
      return;
    }
    console.log(`Creating debug adapter for \`${program}\``);

    await this.client.sendRequest(debugAdapterCreate, {
      sessionId: session.id,
    });
    const debugAdapter = new CandyDebugAdapter(session.id, this.client);
    this.debugAdapters.set(session.id, debugAdapter);
    console.log(`Created debug adapter for session ${session.id}`);

    return new vscode.DebugAdapterInlineImplementation(debugAdapter);
  }

  private resolveProgram(
    program: any,
    workspaceFolder: vscode.WorkspaceFolder | undefined
  ): vscode.Uri | undefined {
    // TODO
    if (!program) {
      vscode.window.showErrorMessage('No `program` specified in `launch.json`');
      return;
    }
    if (typeof program !== 'string') {
      vscode.window.showErrorMessage(
        '`program` specified in `launch.json` must be a string.'
      );
      return;
    }
    program as string;

    if (path.isAbsolute(program)) {
      return vscode.Uri.file(program);
    }

    if (!workspaceFolder) {
      vscode.window.showErrorMessage(
        '`program` specified in `launch.json` must be an absolute path when not in a workspace.'
      );
      return;
    }
    return workspaceFolder.uri.with({
      path: `${workspaceFolder.uri.path}/${path}`,
    });
  }

  dispose() {
    this.onNotificationDisposable.dispose();
  }
}

class CandyDebugAdapter implements vscode.DebugAdapter {
  constructor(
    private readonly debugSessionId: DebugSessionId,
    private readonly client: LanguageClient
  ) {}

  // VS Code → Candy
  handleMessage(message: vscode.DebugProtocolMessage): void {
    console.log(message);
    this.client.sendNotification(debugAdapterMessage, message);
  }

  // VS Code ← Candy
  private readonly onClientNotificationDisposable = this.client.onNotification(
    debugAdapterMessage,
    (it) => this.sendMessage.fire(it)
  );
  private readonly sendMessage =
    new vscode.EventEmitter<vscode.DebugProtocolMessage>();
  onDidSendMessage: vscode.Event<vscode.DebugProtocolMessage> =
    this.sendMessage.event;

  dispose() {
    this.onClientNotificationDisposable.dispose();
  }
}
