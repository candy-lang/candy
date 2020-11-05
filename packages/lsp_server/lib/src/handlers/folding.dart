import 'dart:async';

import 'package:compiler/compiler.dart';

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
    final resourceId = server.fileUriToResourceId(params.textDocument.uri);
    final queryContext = server.queryConfig.createContext();

    final ast = queryContext.callQuery(getAst, resourceId).valueOrNull;
    if (ast == null) {
      return error(
        ErrorCodes.InternalError,
        "Couldn't parse AST of `$resourceId`.",
      );
    }

    final foldingRanges = <FoldingRange>[];
    if (ast.useLines.length > 1) {
      final range = ast.useLines.last.span.toRange(server, resourceId);
      foldingRanges.add(FoldingRange(
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character,
        FoldingRangeKind.Imports,
      ));
    }

    return success(foldingRanges);
  }
}
