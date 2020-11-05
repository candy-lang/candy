import 'dart:async';

import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart';

import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import '../utils.dart';
import 'handlers.dart';

class HoverHandler extends MessageHandler<TextDocumentPositionParams, Hover> {
  HoverHandler(AnalysisServer server) : super(server);

  @override
  Method get handlesMessage => Method.textDocument_hover;
  @override
  LspJsonHandler<TextDocumentPositionParams> get jsonHandler =>
      TextDocumentPositionParams.jsonHandler;

  @override
  Future<ErrorOr<Hover>> handle(
    TextDocumentPositionParams params,
    CancellationToken token,
  ) async {
    final resourceId = server.fileUriToResourceId(params.textDocument.uri);
    final context = server.queryConfig.createContext();

    final ast = context.callQuery(getAst, resourceId).valueOrNull;
    if (ast == null) {
      return error(
        ErrorCodes.InternalError,
        "Couldn't parse AST of `$resourceId`.",
      );
    }

    final offset = params.position.toOffset(server, resourceId);
    final node = NodeFinderVisitor.find(ast, offset);
    final content = MarkupContent(MarkupKind.PlainText, node.toString());
    final range = node.span.toRange(server, resourceId);
    return success(Hover(Either2.t2(content), range));
  }
}
