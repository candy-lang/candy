import 'dart:async';
import 'dart:io';

import 'package:lsp_server/src/analysis_server.dart';

import 'src/channel.dart';

Future<void> main(List<String> arguments) async {
  final channel = LspByteStreamServerChannel(stdin, stdout.nonBlocking);
  final analysisServer = AnalysisServer(channel);
  await channel.closed;

  // Only shutdown the server and exit if the server is not already
  // handling the shutdown.
  if (!analysisServer.willExit) {
    await analysisServer.shutdown();
    exit(0);
  }
}
