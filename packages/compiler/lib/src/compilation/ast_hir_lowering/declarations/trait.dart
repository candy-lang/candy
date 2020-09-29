import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';

extension TraitDeclarationId on DeclarationId {
  bool get isTrait => path.last.data is TraitDeclarationPathData;
  bool get isNotTrait => !isTrait;
}

final getTraitDeclarationAst = Query<DeclarationId, ast.TraitDeclaration>(
  'getTraitDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isTrait);

    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration is ast.TraitDeclaration, 'Wrong return type.');
    return declaration as ast.TraitDeclaration;
  },
);
final getTraitDeclarationHir = Query<DeclarationId, hir.TraitDeclaration>(
  'getTraitDeclarationHir',
  provider: (context, declarationId) {
    final ast = context.callQuery(getTraitDeclarationAst, declarationId);
    final moduleId = context.callQuery(declarationIdToModuleId, declarationId);
    return hir.TraitDeclaration(
      ast.name.name,
      typeParameters: ast.typeParameters.parameters
          .map((p) => hir.TypeParameter(
                name: p.name.name,
                upperBound:
                    astTypeToHirType(context, Tuple2(moduleId, p.bound)),
              ))
          .toList(),
      upperBound: astTypeToHirType(context, Tuple2(moduleId, ast.bound)),
      innerDeclarationIds: getInnerDeclarationIds(
        context,
        moduleIdToDeclarationId(context, moduleId),
      ),
    );
  },
);
