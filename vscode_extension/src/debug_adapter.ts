import * as path from 'path';
import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { CandyDebugAdapterLoggerFactory } from './debug_adapter/logger';
import {
  debugAdapterCreate,
  DebugAdapterId,
  debugAdapterMessage,
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
    DebugAdapterId,
    vscode.DebugAdapter
  >();

  private readonly onNotificationDisposable = this.client.onNotification(
    debugAdapterMessage,
    (message) => {
      const debugAdapter = this.debugAdapters.get(message.debugAdapterId);
      if (!debugAdapter) {
        console.error(
          `No debug adapter found with ID ${message.debugAdapterId}`
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

    const debugAdapterId = await this.client.sendRequest(
      debugAdapterCreate,
      {}
    );
    const debugAdapter = new CandyDebugSession(debugAdapterId, this.client);
    this.debugAdapters.set(debugAdapterId, debugAdapter);
    console.log(`Created debug adapter with ID ${debugAdapterId}`);

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

class CandyDebugSession implements vscode.DebugAdapter {
  constructor(
    private readonly debugAdapterId: DebugAdapterId,
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
