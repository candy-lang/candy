import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext) {
  console.info("Activated the 🍭 Candy extension!");

  const serverOptions = resolveServerOptions();
  if (serverOptions === undefined) return;

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "candy" }],
    outputChannelName: "🍭 Candy Language Server",
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.candy"),
    },
  };

  client = new LanguageClient(
    "candy",
    "🍭 Candy Language Server",
    serverOptions,
    clientOptions,
  );
  context.subscriptions.push(client);

  try {
    await client.start();
  } catch (error) {
    client = undefined;
    await showConfigurationError(
      `Couldn't start the 🍭 Candy language server: ${String(error)}`,
    );
  }
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

function resolveServerOptions(): ServerOptions | undefined {
  const configured = vscode.workspace
    .getConfiguration("candy")
    .get<string>("compilerExecutablePath");
  const hasConfigured = configured !== undefined && configured.length > 0;

  const executablePath = hasConfigured
    ? resolveExecutablePath(configured)
    : undefined;
  if (hasConfigured && executablePath === undefined) {
    void showConfigurationError(
      `Couldn't find the 🍭 Candy compiler at \`${configured}\`.`,
    );
    return undefined;
  }

  // A source checkout of the compiler is a Dart script rather than a binary.
  let command: string;
  let args: string[];
  if (executablePath === undefined) {
    command = "candy";
    args = ["lsp"];
  } else if (executablePath.endsWith(".dart")) {
    command = "dart";
    args = ["run", executablePath, "lsp"];
  } else {
    command = executablePath;
    args = ["lsp"];
  }

  // Spawning a command that doesn't exist tears down the client with a pile of
  // unhandled `EPIPE` rejections, so we'd rather explain what's wrong.
  if (findOnPath(command) === undefined) {
    void showConfigurationError(
      `Couldn't run \`${command}\`: it isn't on the \`PATH\`. Set \`candy.compilerExecutablePath\` to the 🍭 Candy compiler, or to \`bin/candy.dart\` in a compiler checkout.`,
    );
    return undefined;
  }

  return { command, args, transport: TransportKind.stdio };
}

/**
 * A relative path is resolved against the workspace folders, so a checkout can
 * commit one instead of a machine-specific absolute path.
 */
function resolveExecutablePath(configured: string): string | undefined {
  if (path.isAbsolute(configured)) {
    return fs.existsSync(configured) ? configured : undefined;
  }

  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    const candidate = path.join(folder.uri.fsPath, configured);
    if (fs.existsSync(candidate)) return candidate;
  }
  return undefined;
}

function findOnPath(command: string): string | undefined {
  if (command.includes(path.sep)) {
    return fs.existsSync(command) ? command : undefined;
  }

  for (const directory of (process.env.PATH ?? "").split(path.delimiter)) {
    if (directory.length === 0) continue;
    const candidate = path.join(directory, command);
    if (fs.existsSync(candidate)) return candidate;
  }
  return undefined;
}

async function showConfigurationError(message: string) {
  const openSettings = "Open settings";
  const result = await vscode.window.showErrorMessage(message, openSettings);
  if (result === openSettings) {
    await vscode.commands.executeCommand(
      "workbench.action.openSettings",
      "candy.compilerExecutablePath",
    );
  }
}
