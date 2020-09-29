import 'package:compiler/src/compilation/ast_hir_lowering.dart';
import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

import '../../../constants.dart';
import '../../../errors.dart';
import '../../../query.dart';
import '../../../utils.dart';
import '../../ast/parser.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../../ids.dart';
import 'declarations.dart';

const moduleFileName = 'module$candyFileExtension';

extension ModuleResourceId on ResourceId {
  bool get isModuleFile => fileName == moduleFileName;
}

extension ModuleDeclarationId on DeclarationId {
  bool get isModule =>
      path.isEmpty || path.last.data is ModuleDeclarationPathData;
  bool get isNotModule => !isModule;
}

final getModuleDeclarationAst = Query<ModuleId, ast.ModuleDeclaration>(
  'getModuleDeclarationAst',
  provider: (context, moduleId) {
    final declarationId = moduleIdToDeclarationId(context, moduleId);
    final declaration = getDeclarationAst(context, declarationId);
    assert(declaration is ast.ModuleDeclaration, 'Wrong return type.');
    return declaration as ast.ModuleDeclaration;
  },
);
final getModuleDeclarationHir = Query<ModuleId, hir.ModuleDeclaration>(
  'getModuleDeclarationHir',
  provider: (context, moduleId) {
    final ast = getModuleDeclarationAst(context, moduleId);
    return hir.ModuleDeclaration(
      parent: moduleId.parentOrNull,
      name: ast.name.name,
      innerDeclarationIds: getInnerDeclarationIds(
        context,
        moduleIdToDeclarationId(context, moduleId),
      ),
    );
  },
);

final declarationIdToModuleId = Query<DeclarationId, ModuleId>(
  'declarationIdToModuleId',
  persist: false,
  provider: (context, declarationId) {
    final innerPath = declarationId.simplePath
        .whereType<ModuleDeclarationPathData>()
        .map((d) => d.name)
        .toList();
    final base = resourceIdToModuleId(context, declarationId.resourceId);
    return base.nested(innerPath);
  },
);
final moduleIdToDeclarationId = Query<ModuleId, DeclarationId>(
  'moduleIdToDeclarationId',
  provider: (context, moduleId) {
    final declarationId = moduleIdToOptionalDeclarationId(context, moduleId);
    if (declarationId.isNone) {
      throw CompilerError.moduleNotFound('Module `$moduleId` not found.');
    }
    return declarationId.value;
  },
);
final moduleIdToOptionalDeclarationId = Query<ModuleId, Option<DeclarationId>>(
  'moduleIdToOptionalDeclarationId',
  provider: (context, rawModuleId) {
    final moduleId = rawModuleId.normalized;
    final packageId = moduleId.packageId;
    var path = '';
    String pathAnd(String newSegment) =>
        path.isEmpty ? newSegment : '$path/$newSegment';
    final remainingPath = moduleId.path.toList();

    assert(doesResourceDirectoryExist(context, ResourceId(packageId, path)));
    while (remainingPath.isNotEmpty &&
        doesResourceDirectoryExist(
          context,
          ResourceId(packageId, pathAnd(remainingPath.first)),
        )) {
      path = pathAnd(remainingPath.removeAt(0));
    }

    final moduleResourceId = ResourceId(packageId, pathAnd(moduleFileName));
    ResourceId resourceId;
    if (doesResourceExist(context, moduleResourceId)) {
      resourceId = moduleResourceId;
    } else {
      final fileResourceId = ResourceId(
        packageId,
        pathAnd('${remainingPath.first}$candyFileExtension'),
      );
      if (remainingPath.isNotEmpty &&
          doesResourceExist(context, fileResourceId)) {
        resourceId = resourceId = fileResourceId;
        remainingPath.removeAt(0);
      } else {
        return Option.none();
      }
    }

    final declarationId = DeclarationId(
      resourceId,
      remainingPath
          .map((segment) => DisambiguatedDeclarationPathData(
                DeclarationPathData.module(segment),
                0,
              ))
          .toList(),
    );
    return doesDeclarationExist(context, declarationId)
        ? Option.some(declarationId)
        : Option.none();
  },
);
final resourceIdToModuleId = Query<ResourceId, ModuleId>(
  'resourceIdToModuleId',
  persist: false,
  provider: (context, resourceId) {
    assert(resourceId.isCandyFile);

    final path = resourceId.path.removeSuffix(candyFileExtension).split('/');
    return ModuleId(resourceId.packageId, path).normalized;
  },
);

/// Resolves a module given a base [ResourceId] and a [String] as used in a
/// use-line.
final resolveUseLine = Query<Tuple2<ResourceId, ast.UseLine>, ModuleId>(
  'resolveUseLine',
  provider: (context, inputs) {
    final resourceId = inputs.first;
    final useLine = inputs.second;
    // TODO(JonasWanke): packages with slashes
    final modulePathSegments = [
      useLine.packageName.name,
      if (useLine.moduleName != null) useLine.moduleName.name,
    ];

    final currentModuleId = resourceIdToModuleId(context, resourceId);
    final relativeModuleId = currentModuleId.nested(modulePathSegments);
    final relativeDeclarationId =
        moduleIdToOptionalDeclarationId(context, relativeModuleId);
    if (relativeDeclarationId.isSome) return relativeModuleId;

    final absoluteModuleId = ModuleId(
      PackageId(modulePathSegments.first),
      modulePathSegments.skip(1).toList(),
    );
    final absoluteDeclarationId =
        moduleIdToOptionalDeclarationId(context, absoluteModuleId);
    if (absoluteDeclarationId.isSome) return absoluteModuleId;

    assert(false, "Use line `$inputs` couldn't be resolved.");
    return null;
  },
);
