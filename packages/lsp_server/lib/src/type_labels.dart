import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart' as ast;

import 'analysis_server.dart';
import 'generated/lsp_protocol/protocol_custom_generated.dart';
import 'generated/lsp_protocol/protocol_generated.dart';
import 'generated/lsp_protocol/protocol_special.dart';
import 'utils.dart';

const typeLabelsMethod = Method('candy/textDocument/publishTypeLabels');
void updateTypeLabels(
  AnalysisServer server,
  ResourceId resourceId,
) {
  final context = server.queryConfig.createContext();
  final labelsResult =
      context.callQuery(_generateTypeLabels, Tuple2(server, resourceId));
  if (labelsResult is None) {
    server.sendLogMessage(
      'Error computing type labels: ${context.reportedErrors}',
    );
    return;
  }
  final labels = labelsResult.value;

  final params = PublishTypeLabelsParams(
    server.resourceIdToFileUri(resourceId),
    labels,
  );
  final notification =
      NotificationMessage(typeLabelsMethod, params, jsonRpcVersion);
  server.sendNotification(notification);
}

final _generateTypeLabels =
    Query<Tuple2<AnalysisServer, ResourceId>, List<TypeLabel>>(
  'lsp_server.generateTypeLabels',
  provider: (context, inputs) {
    final server = inputs.first;
    final resourceId = inputs.second;

    final declarationIds = getAllDeclarationIds(context, resourceId);
    final labels = <TypeLabel>[];

    for (final id in declarationIds) {
      void handleBody(ast.Expression bodyAst) {
        labels.addAll(_BodyPropertyVisitor.visit(
          context,
          server,
          resourceId,
          lowerBodyAstToHir(context, id).value,
          bodyAst,
        ));
      }

      if (id.isProperty) {
        final propertyAst = getPropertyDeclarationAst(context, id);
        final propertyHir = getPropertyDeclarationHir(context, id);
        if (propertyAst.type == null) {
          final range = propertyAst.name.span.toRange(server, resourceId);
          labels.add(TypeLabel(range, propertyHir.type.toString()));
        }

        if (propertyAst.initializer != null) {
          handleBody(propertyAst.initializer);
        }
      } else if (id.isFunction) {
        final functionAst = getFunctionDeclarationAst(context, id);
        final functionHir = getFunctionDeclarationHir(context, id);
        if (functionAst.returnType == null) {
          final range =
              functionAst.rightParenthesis.span.toRange(server, resourceId);
          labels.add(TypeLabel(range, functionHir.returnType.toString()));
        }

        if (functionAst.body != null) handleBody(functionAst.body);
      }
    }

    return labels;
  },
);

class _BodyPropertyVisitor extends ast.TraversingAstVisitor {
  _BodyPropertyVisitor._(
    this.context,
    this.server,
    this.resourceId,
    this.hirInfos,
  )   : assert(context != null),
        assert(server != null),
        assert(resourceId != null),
        assert(hirInfos != null);
  static List<TypeLabel> visit(
    QueryContext context,
    AnalysisServer server,
    ResourceId resourceId,
    Tuple2<List<Expression>, BodyAstToHirIds> hirInfos,
    ast.Expression bodyAst,
  ) {
    final visitor =
        _BodyPropertyVisitor._(context, server, resourceId, hirInfos);
    bodyAst.accept(visitor);
    return visitor._labels;
  }

  final QueryContext context;
  final AnalysisServer server;
  final ResourceId resourceId;
  final Tuple2<List<Expression>, BodyAstToHirIds> hirInfos;

  final _labels = <TypeLabel>[];

  @override
  void visitPropertyDeclarationExpression(
    ast.PropertyDeclarationExpression node,
  ) {
    if (node.type == null) {
      final id = hirInfos.second.map[node.id];
      final hir = getExpression(context, id).value;
      _labels.add(TypeLabel(
        node.name.span.toRange(server, resourceId),
        hir.type.toString(),
      ));
    }
    super.visitPropertyDeclarationExpression(node);
  }
}
