import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';

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
    final classAst = getClassDeclarationAst(context, declarationId);

    return hir.ClassDeclaration(
      name: classAst.name.name,
      // ignore: can_be_null_after_null_aware
      typeParameters: classAst.typeParameters?.parameters.orEmpty
          .map((p) => hir.TypeParameter(
                name: p.name.name,
                upperBound: p.bound != null
                    ? astTypeToHirType(context, Tuple2(declarationId, p.bound))
                    : hir.CandyType.any,
              ))
          .toList(),
      innerDeclarationIds: getInnerDeclarationIds(context, declarationId) +
          [declarationId.inner(DeclarationPathData.constructor())],
    );
  },
);
