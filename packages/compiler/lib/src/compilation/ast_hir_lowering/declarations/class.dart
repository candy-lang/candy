import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';

extension ClassDeclarationId on DeclarationId {
  bool get isClass =>
      path.isNotEmpty && path.last.data is ClassDeclarationPathData;
  bool get isNotClass => !isClass;
}

final getClassDeclarationAst = Query<DeclarationId, ast.ClassDeclaration>(
  'getClassDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isClass);

    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration is ast.ClassDeclaration, 'Wrong return type.');
    return declaration as ast.ClassDeclaration;
  },
);
final getClassDeclarationHir = Query<DeclarationId, hir.ClassDeclaration>(
  'getClassDeclarationHir',
  provider: (context, declarationId) {
    final ast = getClassDeclarationAst(context, declarationId);
    final moduleId = declarationIdToModuleId(context, declarationId);
    return hir.ClassDeclaration(
      name: ast.name.name,
      // ignore: can_be_null_after_null_aware
      typeParameters: ast.typeParameters?.parameters.orEmpty
          .map((p) => hir.TypeParameter(
                name: p.name.name,
                upperBound: p.bound != null
                    ? astTypeToHirType(context, Tuple2(moduleId, p.bound))
                    : hir.CandyType.any,
              ))
          .toList(),
      innerDeclarationIds: getInnerDeclarationIds(
            context,
            moduleIdToDeclarationId(context, moduleId),
          ) +
          [declarationId.inner(DeclarationPathData.constructor())],
    );
  },
);
