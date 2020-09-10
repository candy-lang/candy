import 'dart:async';
import 'dart:io';

import 'package:parser/parser.dart';

import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import '../utils.dart';
import 'handlers.dart';

class FoldingHandler
    extends MessageHandler<FoldingRangeParams, List<FoldingRange>> {
  FoldingHandler(AnalysisServer server) : super(server);

  @override
  Method get handlesMessage => Method.textDocument_foldingRange;

  @override
  LspJsonHandler<FoldingRangeParams> get jsonHandler =>
      FoldingRangeParams.jsonHandler;

  @override
  Future<ErrorOr<List<FoldingRange>>> handle(
    FoldingRangeParams params,
    CancellationToken token,
  ) async {
    final uri = params.textDocument.uri;
    if (!isCandyDocument(uri)) {
      return error(
        ErrorCodes.InvalidParams,
        'File $uri is not a Candy source file.',
      );
    }

    final source = File.fromUri(Uri.parse(uri)).readAsStringSync();
    final ast = parseCandySource(source);
    final foldingRanges = <FoldingRange>[];

    if (ast.useLines.length > 1) {
      final useLinesStart = positionOf(source, ast.useLines.first.span.start);
      final useLinesEnd = positionOf(source, ast.useLines.last.span.end);
      foldingRanges.add(FoldingRange(
        useLinesStart.line,
        useLinesStart.character,
        useLinesEnd.line,
        useLinesEnd.character,
        FoldingRangeKind.Imports,
      ));
    }

    return success(foldingRanges);
  }
}
