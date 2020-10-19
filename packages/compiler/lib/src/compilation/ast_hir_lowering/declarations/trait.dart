import 'package:parser/parser.dart' as ast;

import '../../../errors.dart';
import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';

extension TraitDeclarationId on DeclarationId {
  bool get isTrait =>
      path.isNotEmpty && path.last.data is TraitDeclarationPathData;
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

    List<hir.UserCandyType> upperBounds;
    if (ast.bound != null) {
      final upperBoundType =
          astTypeToHirType(context, Tuple2(moduleId, ast.bound));
      upperBounds = hirTypeToUserTypes(
        context,
        upperBoundType,
        ErrorLocation(declarationId.resourceId, ast.bound.span),
      );
    }

    return hir.TraitDeclaration(
      ast.name.name,
      // ignore: can_be_null_after_null_aware
      typeParameters: ast.typeParameters?.parameters.orEmpty
          .map((p) => hir.TypeParameter(
                name: p.name.name,
                upperBound: p.bound != null
                    ? astTypeToHirType(context, Tuple2(moduleId, p.bound))
                    : null,
              ))
          .toList(),
      upperBounds: upperBounds,
      innerDeclarationIds: getInnerDeclarationIds(context, declarationId),
    );
  },
);

List<hir.UserCandyType> hirTypeToUserTypes(
  QueryContext context,
  hir.CandyType type,
  ErrorLocation location,
) {
  List<hir.CandyType> traits;
  if (type is hir.UserCandyType) {
    return [type];
  } else if (type is hir.IntersectionCandyType) {
    traits = type.types;
  }

  if (traits == null || traits.any((t) => t is! hir.UserCandyType)) {
    throw CompilerError.invalidImplTraitBound(
      'Impl trait bound must be a simple type or an intersection type.',
      location: location,
    );
  }
  return traits.cast<hir.UserCandyType>();
}
