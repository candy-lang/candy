import 'package:parser/parser.dart' as ast;

import '../../../candyspec.dart';
import '../../../errors.dart';
import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../../ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';
import 'trait.dart';

extension ImplDeclarationId on DeclarationId {
  bool get isImpl =>
      path.isNotEmpty && path.last.data is ImplDeclarationPathData;
  bool get isNotImpl => !isImpl;
}

final getImplDeclarationAst = Query<DeclarationId, ast.ImplDeclaration>(
  'getImplDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isImpl);

    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration is ast.ImplDeclaration, 'Wrong return type.');
    return declaration as ast.ImplDeclaration;
  },
);
final getImplDeclarationHir = Query<DeclarationId, hir.ImplDeclaration>(
  'getImplDeclarationHir',
  provider: (context, declarationId) {
    final implAst = context.callQuery(getImplDeclarationAst, declarationId);

    // ignore: can_be_null_after_null_aware
    final typeParameters = implAst.typeParameters?.parameters.orEmpty
        .map((p) => hir.TypeParameter(
              name: p.name.name,
              upperBound: p.bound != null
                  ? astTypeToHirType(context, Tuple2(declarationId, p.bound))
                  : hir.CandyType.any,
            ))
        .toList();

    // TODO(JonasWanke): check impl validity (required methods available, correct package)

    return hir.ImplDeclaration(
      typeParameters: typeParameters,
      type: astTypeToHirType(context, Tuple2(declarationId, implAst.type))
          as hir.UserCandyType,
      traits: hirTypeToUserTypes(
        context,
        astTypeToHirType(context, Tuple2(declarationId, implAst.trait)),
        ErrorLocation(declarationId.resourceId, implAst.trait.span),
      ),
      innerDeclarationIds: getInnerDeclarationIds(context, declarationId),
    );
  },
);

final getAllImplsForType =
    Query<Tuple2<hir.CandyType, PackageId>, List<DeclarationId>>(
  'getAllImplsForType',
  provider: (context, inputs) {
    final type = inputs.first;
    final packageId = inputs.second;

    return context.config.resourceProvider
        .getAllFileResourceIds(context, packageId)
        .where((resourceId) => resourceId.isCandySourceFile)
        .expand((resourceId) =>
            _getImplDeclarationIds(context, DeclarationId(resourceId)))
        .where((id) => getImplDeclarationHir(context, id).type == type)
        .toList();
  },
);

Iterable<DeclarationId> _getImplDeclarationIds(
  QueryContext context,
  DeclarationId declarationId,
) sync* {
  if (declarationId.isImpl) {
    yield declarationId;
  } else if (declarationId.isModule) {
    final moduleId = declarationIdToModuleId(context, declarationId);
    yield* getModuleDeclarationHir(context, moduleId)
        .innerDeclarationIds
        .expand((id) => _getImplDeclarationIds(context, id));
  }
}

final getAllImplsForClass = Query<DeclarationId, List<DeclarationId>>(
  'getAllImplsForClass',
  provider: (context, declarationId) {
    return getAllDependencies(context, Unit())
        .followedBy([context.config.packageId])
        .expand((packageId) => context.config.resourceProvider
            .getAllFileResourceIds(context, packageId))
        .where((resourceId) => resourceId.isCandySourceFile)
        .expand((resourceId) =>
            _getImplDeclarationIds(context, DeclarationId(resourceId)))
        .where((id) {
          final moduleId =
              getImplDeclarationHir(context, id).type.virtualModuleId;
          return moduleIdToDeclarationId(context, moduleId) == declarationId;
        })
        .toList();
  },
);
