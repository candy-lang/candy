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
  bool get isNotFunction => !isFunction;
}

final getFunctionDeclarationAst = Query<DeclarationId, ast.FunctionDeclaration>(
  'getFunctionDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isFunction);

    final declaration = getDeclarationAst(context, declarationId);
    assert(declaration is ast.FunctionDeclaration, 'Wrong return type.');
    return declaration as ast.FunctionDeclaration;
  },
);
final getFunctionDeclarationHir = Query<DeclarationId, hir.FunctionDeclaration>(
  'getFunctionDeclarationHir',
  provider: (context, declarationId) {
    final ast = getFunctionDeclarationAst(context, declarationId);
    final moduleId = declarationIdToModuleId(context, declarationId);
    return hir.FunctionDeclaration(
      name: ast.name.name,
      parameters: ast.valueParameters
          .map((p) => hir.ValueParameter(
                name: p.name.name,
                type: astTypeToHirType(context, Tuple2(moduleId, p.type)),
              ))
          .toList(),
      returnType: ast.returnType != null
          ? astTypeToHirType(context, Tuple2(moduleId, ast.returnType))
          : hir.CandyType.unit,
      // TODO(JonasWanke): child declarations
    );
  },
);
