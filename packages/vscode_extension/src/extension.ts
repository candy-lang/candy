import * as child_process from "child_process";
import * as stream from "stream";
import * as vs from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  StreamInfo,
} from "vscode-languageclient";

let client: LanguageClient;

export async function activate(context: vs.ExtensionContext) {
  console.log("Activated 🍭 Candy extension!");

  let clientOptions: LanguageClientOptions = {
    outputChannelName: "Candy Analysis Server",
  };

  client = new LanguageClient(
    "candyAnalysisLSP",
    "Candy Analysis Server",
    spawnServer,
    clientOptions
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

// The following code is taken (and slightly modified) from https://github.com/Dart-Code/Dart-Code
async function spawnServer(): Promise<StreamInfo> {
  const process = safeSpawn(
    undefined,
    "C:\\Program Files\\Dart\\dart-sdk\\bin\\dart.exe",
    ["C:/p/candy/packages/lsp_server/bin/main.dart"],
    {}
  );
  console.info(`    PID: ${process.pid}`);

  const reader = process.stdout.pipe(new LoggingTransform("<=="));
  const writer = new LoggingTransform("==>");
  writer.pipe(process.stdin);

  process.stderr.on("data", (data) => console.error(data.toString()));

  return { reader, writer };
}

type SpawnedProcess = child_process.ChildProcess & {
  stdin: stream.Writable;
  stdout: stream.Readable;
  stderr: stream.Readable;
};
function safeSpawn(
  workingDirectory: string | undefined,
  binPath: string,
  args: string[],
  env: {
    envOverrides?: { [key: string]: string | undefined };
    toolEnv?: { [key: string]: string | undefined };
  }
): SpawnedProcess {
  // Spawning processes on Windows with funny symbols in the path requires quoting. However if you quote an
  // executable with a space in its path and an argument also has a space, you have to then quote all of the
  // arguments too!\
  // https://github.com/nodejs/node/issues/7367
  const customEnv = Object.assign(
    {},
    process.env,
    env.toolEnv,
    env.envOverrides
  );
  const quotedArgs = args.map((a) => `"${a.replace(/"/g, `\\"`)}"`);
  return child_process.spawn(`"${binPath}"`, quotedArgs, {
    cwd: workingDirectory,
    env: customEnv,
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
    let value = (chunk as Buffer).toString();
    if (value.startsWith("Observatory listening on")) {
      console.warn(value);
      callback();
      return;
    }

    let toLog = this.onlyShowJson
      ? value
          .split("\r\n")
          .filter(
            (line) => line.trim().startsWith("{") || line.trim().startsWith("#")
          )
          .join("\r\n")
      : value;
    if (toLog.length > 0 || !this.onlyShowJson) {
      console.info(`${this.prefix} ${toLog}`);
    }

    this.push(chunk, encoding);
    callback();
  }
}
