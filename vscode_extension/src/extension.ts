import * as child_process from "child_process";
import * as stream from "stream";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  StreamInfo,
} from "vscode-languageclient/node";
import { registerDebugAdapter } from "./debug_adapter";
import { registerDebugIrCommands } from "./debug_irs";
import { HintsDecorations } from "./hints";
import { ServerStatusService } from "./server_status";

let client: LanguageClient | undefined;
const enableLogging = false;

export async function activate(context: vscode.ExtensionContext) {
  console.log("Activated 🍭 Candy extension!");

  const configuration = vscode.workspace.getConfiguration("candy");
  const packagesPath = configuration.get<string>("packagesPath");
  if (!packagesPath) {
    const result = await vscode.window.showErrorMessage(
      "Please configure the setting `candy.packagesPath` and reload this window.",
      "Open settings",
    );
    if (result) {
      await vscode.commands.executeCommand(
        "workbench.action.openSettings",
        "candy.packagesPath",
      );
    }
    return;
  }

  const clientOptions: LanguageClientOptions = {
    outputChannelName: "🍭 Candy Language Server",
    initializationOptions: { packagesPath },
  };

  client = new LanguageClient(
    "candyLanguageServer",
    "Candy Language Server",
    spawnServer,
    clientOptions,
  );
  await client.start();

  context.subscriptions.push(new ServerStatusService(client));
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
function spawnServer(): Promise<StreamInfo> {
  const process = safeSpawn();
  console.info(`PID: ${process.pid}`);

  let reader = process.stdout;
  let writer = process.stdin;

  // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
  if (enableLogging) {
    reader = process.stdout.pipe(new LoggingTransform("<=="));
    writer = new LoggingTransform("==>");
    writer.pipe(process.stdin);
  }

  process.stderr.on("data", (data) => {
    console.error(String(data));
  });

  process.addListener("close", (exitCode) => {
    if (exitCode === 101) {
      console.error("LSP server was closed with a panic.");
    } else {
      console.error(`LSP server was closed with code ${exitCode}.`);
    }
  });
  process.addListener("disconnect", () => {
    console.error("LSP server disconnected.");
  });
  process.addListener("error", (event) => {
    console.error(`LSP server had an error: ${event.toString()}`);
  });
  process.addListener("exit", (exitCode) => {
    if (exitCode === 101) {
      console.error("LSP server panicked.");
    } else {
      console.error(`LSP server exited with exit code ${exitCode}.`);
    }
  });
  process.addListener("message", () => {
    console.error("LSP server sent a message.");
  });

  return Promise.resolve({ reader, writer });
}

type SpawnedProcess = child_process.ChildProcess & {
  stdin: stream.Writable;
  stdout: stream.Readable;
  stderr: stream.Readable;
};
function safeSpawn(): SpawnedProcess {
  const configuration = vscode.workspace.getConfiguration("candy");

  let command: [string, string[]] = ["candy", ["lsp"]];
  const languageServerCommand = configuration.get<string>(
    "languageServerCommand",
  );
  if (languageServerCommand && languageServerCommand.trim().length !== 0) {
    const parts = languageServerCommand.split(" ");
    command = [parts[0], parts.slice(1)];
  }

  return child_process.spawn(command[0], command[1], {
    cwd: vscode.workspace.rootPath,
    env: { ...process.env, RUST_BACKTRACE: "FULL" },
    shell: true,
  }) as SpawnedProcess;
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
class LoggingTransform extends stream.Transform {
  constructor(
    private readonly prefix: string,
    opts?: stream.TransformOptions,
  ) {
    super(opts);
  }
  public _transform(
    chunk: unknown,
    encoding: BufferEncoding,
    callback: () => void,
  ): void {
    const value = (chunk as Buffer).toString();
    const toLog = value
      .split("\r\n")
      .filter(
        (line) => line.trim().startsWith("{") || line.trim().startsWith("#"),
      )
      .join("\r\n");
    if (toLog.length > 0) {
      console.info(`${this.prefix} ${toLog}`);
    }

    this.push(Buffer.from(value, "utf8"), encoding);
    callback();
  }
}
