import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';

extension FunctionDeclarationId on DeclarationId {
  bool get isFunction =>
      path.isNotEmpty && path.last.data is FunctionDeclarationPathData;
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
    final functionAst = getFunctionDeclarationAst(context, declarationId);

    return hir.FunctionDeclaration(
      isStatic: functionAst.isStatic || declarationId.parent.isModule,
      name: functionAst.name.name,
      // ignore: can_be_null_after_null_aware
      typeParameters: functionAst.typeParameters?.parameters.orEmpty
          .map((p) => hir.TypeParameter(
                name: p.name.name,
                upperBound: p.bound != null
                    ? astTypeToHirType(
                        context,
                        Tuple2(
                          declarationId.parent,
                          p.bound,
                        ))
                    : hir.CandyType.any,
              ))
          .toList(),
      valueParameters: functionAst.valueParameters
          .map((p) => hir.ValueParameter(
                name: p.name.name,
                type: astTypeToHirType(
                  context,
                  Tuple2(declarationId.parent, p.type),
                ),
              ))
          .toList(),
      returnType: functionAst.returnType != null
          ? astTypeToHirType(
              context,
              Tuple2(declarationId.parent, functionAst.returnType),
            )
          : hir.CandyType.unit,
    );
  },
);
