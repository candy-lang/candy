import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

import '../../../candyspec.dart';
import '../../../errors.dart';
import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../../ids.dart';
import '../type.dart';
import 'class.dart';
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

    final declaration = getDeclarationAst(context, declarationId);
    assert(declaration is ast.ImplDeclaration, 'Wrong return type.');
    return declaration as ast.ImplDeclaration;
  },
);
final getImplDeclarationHir = Query<DeclarationId, hir.ImplDeclaration>(
  'getImplDeclarationHir',
  provider: (context, declarationId) {
    if (!doesDeclarationExist(context, declarationId)) {
      final classId = declarationId.parent;
      assert(classId.isClass);
      final classHir = getClassDeclarationHir(context, classId);

      final index = declarationId.path.last.disambiguator;
      return classHir.syntheticImpls[index].implHir;
    }
    final implAst = getImplDeclarationAst(context, declarationId);

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

final getAllImplsForType = Query<hir.CandyType, List<DeclarationId>>(
  'getAllImplsForType',
  provider: (context, type) {
    assert(type is hir.UserCandyType);
    final impls = getAllImpl(context, Unit())
        .where((id) =>
            // TODO(marcelgarus): Constraint solving should go here.
            getImplDeclarationHir(context, id).type.name ==
            (type as hir.UserCandyType).name)
        .toList();

    if (type is hir.UserCandyType) {
      final declarationId =
          moduleIdToDeclarationId(context, type.virtualModuleId);
      if (declarationId.isClass) {
        impls.addAll(getSyntheticImplIdsForClass(context, declarationId));
      }
    }

    return impls;
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

final getAllImplsForTraitOrClass = Query<DeclarationId, List<DeclarationId>>(
  'getAllImplsForTraitOrClass',
  provider: (context, declarationId) {
    final impls = getAllImpl(context, Unit()).where((id) {
      final moduleId = getImplDeclarationHir(context, id).type.virtualModuleId;
      return moduleIdToDeclarationId(context, moduleId) == declarationId;
    }).toList();

    if (declarationId.isClass) {
      impls.addAll(getSyntheticImplIdsForClass(context, declarationId));
    }

    return impls;
  },
);

final getAllImpl = Query<Unit, List<DeclarationId>>(
  'getAllImpl',
  provider: (context, declarationId) {
    return getAllDependencies(context, Unit())
        .followedBy([
          if (context.config.packageId != PackageId.core)
            context.config.packageId,
        ])
        .expand((packageId) => context.config.resourceProvider
            .getAllFileResourceIds(context, packageId))
        .where((resourceId) => resourceId.isCandySourceFile)
        .expand((resourceId) =>
            _getImplDeclarationIds(context, DeclarationId(resourceId)))
        .distinct()
        .toList();
  },
);
final getSyntheticImplIdsForClass = Query<DeclarationId, List<DeclarationId>>(
  'getSyntheticImplIdsForClass',
  provider: (context, classId) {
    assert(classId.isClass);
    final classHir = getClassDeclarationHir(context, classId);
    return classHir.syntheticImpls.indices
        .map((it) => classId.inner(DeclarationPathData.impl('synthetic'), it))
        .toList();
  },
);
