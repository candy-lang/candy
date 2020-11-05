import 'dart:async';

import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart' show NodeFinderVisitor, SourceSpan;
import 'package:parser/parser.dart' as ast;

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

    final fileAst = context.callQuery(getAst, resourceId).valueOrNull;
    if (fileAst == null) {
      return error(
        ErrorCodes.InternalError,
        "Couldn't parse AST of `$resourceId`.",
      );
    }

    final offset = params.position.toOffset(server, resourceId);
    final nodeAst = NodeFinderVisitor.find(fileAst, offset);
    var content = 'AST: $nodeAst';
    if (nodeAst is ast.Expression) {
      final nodeHir = context.callQuery(
        getExpressionFromAstId,
        Tuple2(resourceId, nodeAst.id),
      );
      assert(nodeHir is Some);
      final type = nodeHir.value.value.type.toString();
      content = '```candy\n'
          '$type\n'
          '```\n'
          '\n'
          '---\n'
          '\n'
          'HIR: $nodeHir\n'
          '\n'
          '---\n'
          '\n'
          '$content';
    }

    final range = nodeAst.span.toRange(server, resourceId);
    return success(Hover(
      Either2.t2(MarkupContent(MarkupKind.Markdown, content)),
      range,
    ));
  }
}
