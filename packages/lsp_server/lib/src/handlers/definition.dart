import 'dart:async';

import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart' as ast;

import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import '../utils.dart';
import 'handlers.dart';

class DefinitionHandler
    extends MessageHandler<TextDocumentPositionParams, List<LocationLink>> {
  DefinitionHandler(AnalysisServer server) : super(server);

  @override
  Method get handlesMessage => Method.textDocument_definition;
  @override
  LspJsonHandler<TextDocumentPositionParams> get jsonHandler =>
      TextDocumentPositionParams.jsonHandler;

  @override
  Future<ErrorOr<List<LocationLink>>> handle(
    TextDocumentPositionParams params,
    CancellationToken token,
  ) async {
    if (!isCandyDocument(params.textDocument.uri)) return success(const []);

    final resourceId = server.fileUriToResourceId(params.textDocument.uri);
    final context = server.queryConfig.createContext();

    final astNodeResult =
        getAstNodeAtPosition(server, resourceId, params.position);
    if (astNodeResult is Error) {
      return error(ErrorCodes.InternalError, astNodeResult.error);
    }
    final astNode = astNodeResult.value;
    final originalSelectionRange = astNode.span.toRange(server, resourceId);

    final expressionHirResult =
        getExpressionHirAtPosition(server, resourceId, params.position);
    if (expressionHirResult is Error) {
      return error(ErrorCodes.InternalError, expressionHirResult.error);
    }
    final expressionHirOption = expressionHirResult.value;
    if (expressionHirOption is None) return success(<LocationLink>[]);
    final expressionHir = expressionHirOption.value;

    Result<List<LocationLink>, String> resolve(DeclarationId id) {
      final declarationResult = context.callQuery(getDeclarationAst, id);
      if (declarationResult is None) {
        return Error(
          'Error while resolving declaration ID $id: ${context.reportedErrors}',
        );
      }
      final declarationAst = declarationResult.value;

      return Ok([
        LocationLink(
          originalSelectionRange,
          server.resourceIdToFileUri(id.resourceId),
          declarationAst.span.toRange(server, resourceId),
          declarationAst.representativeSpan.toRange(server, resourceId),
        ),
      ]);
    }

    Result<List<LocationLink>, String> resolveLocal(DeclarationLocalId id) {
      final loweringResult =
          context.callQuery(getBodyAstToHirIds, id.declarationId);
      if (loweringResult is None) {
        return Error(
          'Error while getting body AST to HIR IDs of $id: ${context.reportedErrors}',
        );
      }
      final astToHirIds = loweringResult.value.value;
      final astId =
          astToHirIds.map.entries.firstWhere((it) => it.value == id).key;

      final definitionResourceId = id.declarationId.resourceId;
      final fileAstResult = context.callQuery(getAst, definitionResourceId);
      if (fileAstResult is None) {
        return Error(
          'Error while retrieving file AST of $definitionResourceId: ${context.reportedErrors}',
        );
      }
      final fileAst = fileAstResult.value;

      final astNode = ast.ExpressionFinderVisitor.find(fileAst, astId);
      if (astNode == null) {
        return Error("Couldn't find AST node with ID $astId.");
      }

      final fullRange = astNode.span.toRange(server, definitionResourceId);
      return Ok([
        LocationLink(
          originalSelectionRange,
          server.resourceIdToFileUri(definitionResourceId),
          fullRange,
          astNode is ast.PropertyDeclarationExpression
              ? astNode.name.span.toRange(server, resourceId)
              : fullRange,
        ),
      ]);
    }

    // ignore: omit_local_variable_types
    final Result<List<LocationLink>, String> result = expressionHir.maybeMap(
      identifier: (it) => it.identifier.maybeMap(
        reflection: (it) => resolve(it.id),
        parameter: (param) {
          final functionAstResult = context.callQuery(
              getFunctionDeclarationAst, param.id.declarationId);
          if (functionAstResult is None) {
            return Error(
              'Error while getting function AST of ${param.id.declarationId}: ${context.reportedErrors}',
            );
          }
          final functionAst = functionAstResult.value;

          final parameterAst = functionAst.valueParameters
              .firstWhere((it) => it.name.name == param.name);
          return Ok([
            LocationLink(
              originalSelectionRange,
              server.resourceIdToFileUri(param.id.declarationId.resourceId),
              parameterAst.span.toRange(server, resourceId),
              parameterAst.name.span.toRange(server, resourceId),
            ),
          ]);
        },
        property: (it) => resolve(it.id),
        localProperty: (it) => resolveLocal(it.id),
        orElse: () => Ok([]),
      ),
      navigation: null,
      return_: (it) => resolveLocal(it.scopeId),
      break_: (it) => resolveLocal(it.scopeId),
      continue_: (it) => resolveLocal(it.scopeId),
      orElse: () => Ok([]),
    );
    if (result is Error) return error(ErrorCodes.InternalError, result.error);
    return success(result.value);
  }
}
