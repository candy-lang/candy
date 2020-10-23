import 'dart:async';
import 'dart:io';

import 'package:args/args.dart';
import 'package:lsp_server/src/analysis_server.dart';
import 'package:lsp_server/src/channel.dart';

const _optionCandyDirectory = 'candy-path';

Future<void> main(List<String> arguments) async {
  stderr.write('Starting LSP…');

  final parser = ArgParser()..addOption(_optionCandyDirectory);
  final results = parser.parse(arguments);

  final candyDirectory = Directory(results[_optionCandyDirectory]);
  if (!candyDirectory.existsSync()) {
    exit(HttpStatus.notFound);
  }

  final channel = LspByteStreamServerChannel(stdin, stdout);
  final analysisServer = AnalysisServer(channel, candyDirectory);
  stderr.write('Started LSP.');
  await channel.closed;

  // Only shutdown the server and exit if the server is not already
  // handling the shutdown.
  if (!analysisServer.willExit) {
    await analysisServer.shutdown();
    exit(0);
  }
}
