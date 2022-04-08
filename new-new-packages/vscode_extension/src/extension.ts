import * as child_process from 'child_process';
import * as stream from 'stream';
import * as vs from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  StreamInfo,
} from 'vscode-languageclient/node';
import { HintsDecorations } from './hints';

let client: LanguageClient;

export async function activate(context: vs.ExtensionContext) {
  console.log('Activated 🍭 Candy extension!');

  let clientOptions: LanguageClientOptions = {
    outputChannelName: '🍭 Candy Language Server',
  };

  client = new LanguageClient(
    'candyLanguageServer',
    'Candy Language Server',
    spawnServer,
    clientOptions
  );
  client.start();

  context.subscriptions.push(new HintsDecorations(client));
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

  const reader = process.stdout.pipe(new LoggingTransform('<=='));
  const writer = new LoggingTransform('==>');
  writer.pipe(process.stdin);

  process.stderr.on('data', (data) => console.error(data.toString()));

  process.addListener('close', (exitCode) => {
    if (exitCode == 101) {
      console.error('LSP server was closed with a panic.');
    } else {
      console.error('LSP server was closed with code ' + exitCode + '.');
    }
  });
  process.addListener('disconnect', () => {
    console.error('LSP server disconnected.');
  });
  process.addListener('error', (event) => {
    console.error('LSP server had an error: ' + event);
  });
  process.addListener('exit', (exitCode) => {
    if (exitCode == 101) {
      console.error('LSP server panicked.');
    } else {
      console.error('LSP server exited with exit code ' + exitCode + '.');
    }
  });
  process.addListener('message', () => {
    console.error('LSP server sent a message.');
  });

  return { reader, writer };
}

type SpawnedProcess = child_process.ChildProcess & {
  stdin: stream.Writable;
  stdout: stream.Readable;
  stderr: stream.Readable;
};
function safeSpawn(): SpawnedProcess {
  const configuration = vs.workspace.getConfiguration('candy');

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
    env: { ...process.env, RUST_BACKTRACE: '1' },
    shell: true,
  }) as SpawnedProcess;
}
class LoggingTransform extends stream.Transform {
  constructor(
    private readonly prefix: string,
    private readonly onlyShowJson: boolean = true,
    opts?: stream.TransformOptions
  ) {
    super(opts);
  }
  public _transform(
    chunk: any,
    encoding: BufferEncoding,
    callback: () => void
  ): void {
    const value = (chunk as Buffer).toString();
    const toLog = this.onlyShowJson
      ? value
          .split('\r\n')
          .filter(
            (line) => line.trim().startsWith('{') || line.trim().startsWith('#')
          )
          .join('\r\n')
      : value;
    if (toLog.length > 0 || !this.onlyShowJson) {
      console.info(`${this.prefix} ${toLog}`);
    }

    // TODO: This is a workaround because VSCode doesn't adhere to the LSP spec.
    const fixedValue = value.replace(
      '"prepareSupportDefaultBehavior":true',
      '"prepareSupportDefaultBehavior":   1'
    );
    this.push(Buffer.from(fixedValue, 'utf8'), encoding);
    callback();
  }
}
