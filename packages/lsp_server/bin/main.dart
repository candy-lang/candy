import 'dart:async';
import 'dart:io';

import 'package:args/args.dart';
import 'package:lsp_server/src/analysis_server.dart';
import 'package:lsp_server/src/channel.dart';

const _optionCoreDirectory = 'core-path';

Future<void> main(List<String> arguments) async {
  stderr.write('Starting LSP…');

  final parser = ArgParser()..addOption(_optionCoreDirectory);
  final results = parser.parse(arguments);

  final corePath = results[_optionCoreDirectory];
  final coreDirectory = Directory(corePath);
  if (!coreDirectory.existsSync()) {
    stderr.write('Core library not found at $corePath.');
    exit(HttpStatus.notFound);
  }

  final channel = LspByteStreamServerChannel(stdin, stdout);
  final analysisServer = AnalysisServer(channel, coreDirectory);
  stderr.write('Started LSP.');
  await channel.closed;

  // Only shutdown the server and exit if the server is not already
  // handling the shutdown.
  if (!analysisServer.willExit) {
    await analysisServer.shutdown();
    exit(0);
  }
}
