import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';

extension FunctionDeclarationId on DeclarationId {
  bool get isFunction => path.last.data is FunctionDeclarationPathData;
}

final getFunctionDeclarationAst = Query<DeclarationId, ast.FunctionDeclaration>(
  'getFunctionDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isFunction);

    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration != null, 'Function $declarationId not found.');
    assert(declaration is ast.FunctionDeclaration, 'Wrong return type.');
    return declaration as ast.FunctionDeclaration;
  },
);
final getFunctionDeclarationHir = Query<DeclarationId, hir.FunctionDeclaration>(
  'getFunctionDeclarationHir',
  provider: (context, declarationId) {
    final moduleId = context.callQuery(declarationIdToModuleId, declarationId);

    final ast = context.callQuery(getFunctionDeclarationAst, declarationId);
    return hir.FunctionDeclaration(
      name: ast.name.name,
      parameters: ast.valueParameters
          .map((p) => hir.FunctionParameter(
                name: p.name.name,
                type: context.callQuery(astTypeToHirType, Tuple2(moduleId, p)),
              ))
          .toList(),
      returnType:
          context.callQuery(astTypeToHirType, Tuple2(moduleId, ast.returnType)),
      // TODO(JonasWanke): child declarations
    );
  },
);
