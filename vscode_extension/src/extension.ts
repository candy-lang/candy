import * as child_process from 'child_process';
import * as stream from 'stream';
import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  StreamInfo,
} from 'vscode-languageclient/node';
import { registerDebugAdapter } from './debug_adapter';
import { registerDebugIrCommands } from './debug_irs';
import { HintsDecorations } from './hints';

let client: LanguageClient;

export async function activate(context: vscode.ExtensionContext) {
  console.log('Activated 🍭 Candy extension!');

  const configuration = vscode.workspace.getConfiguration('candy');
  const packagesPath = configuration.get<string>('packagesPath');
  if (!packagesPath) {
    const result = await vscode.window.showErrorMessage(
      'Please configure the setting `candy.packagesPath` and reload this window.',
      'Open settings'
    );
    if (result) {
      vscode.commands.executeCommand(
        'workbench.action.openSettings',
        'candy.packagesPath'
      );
    }
    return;
  }

  let clientOptions: LanguageClientOptions = {
    outputChannelName: '🍭 Candy Language Server',
    initializationOptions: { packagesPath },
  };

  client = new LanguageClient(
    'candyLanguageServer',
    'Candy Language Server',
    spawnServer,
    clientOptions
  );
  client.start();

  context.subscriptions.push(new HintsDecorations(client));
  registerDebugIrCommands(client);
  registerDebugAdapter(context, client);
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }

  return client.stop();
}

// The following code is taken (and slightly modified) from https://github.com/Dart-Code/Dart-Code
async function spawnServer(): Promise<StreamInfo> {
  const process = safeSpawn();
  console.info(`PID: ${process.pid}`);

  process.stderr.on('data', (data) => console.error(data.toString()));

  process.addListener('close', (exitCode) => {
    if (exitCode === 101) {
      console.error('LSP server was closed with a panic.');
    } else {
      console.error(`LSP server was closed with code ${exitCode}.`);
    }
  });
  process.addListener('disconnect', () => {
    console.error('LSP server disconnected.');
  });
  process.addListener('error', (event) => {
    console.error(`LSP server had an error: ${event}`);
  });
  process.addListener('exit', (exitCode) => {
    if (exitCode === 101) {
      console.error('LSP server panicked.');
    } else {
      console.error(`LSP server exited with exit code ${exitCode}.`);
    }
  });
  process.addListener('message', () => {
    console.error('LSP server sent a message.');
  });

  return { reader: process.stdout, writer: process.stdin };
}

type SpawnedProcess = child_process.ChildProcess & {
  stdin: stream.Writable;
  stdout: stream.Readable;
  stderr: stream.Readable;
};
function safeSpawn(): SpawnedProcess {
  const configuration = vscode.workspace.getConfiguration('candy');

  let command: [string, string[]] = ['candy', ['lsp']];
  const languageServerCommand = configuration.get<string>(
    'languageServerCommand'
  );
  if (languageServerCommand && languageServerCommand.trim().length !== 0) {
    const parts = languageServerCommand.split(' ');
    command = [parts[0], parts.slice(1)];
  }

  return child_process.spawn(command[0], command[1], {
    // eslint-disable-next-line @typescript-eslint/naming-convention
    env: { ...process.env, RUST_BACKTRACE: 'FULL' },
    shell: true,
  }) as SpawnedProcess;
}
