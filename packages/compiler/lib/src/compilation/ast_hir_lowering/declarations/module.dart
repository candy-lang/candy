import 'package:compiler/src/compilation/ast_hir_lowering.dart';
import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

import '../../../constants.dart';
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
    final declarationId = context.callQuery(moduleIdToDeclarationId, moduleId);
    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration is ast.ModuleDeclaration, 'Wrong return type.');
    return declaration as ast.ModuleDeclaration;
  },
);
final getModuleDeclarationHir = Query<ModuleId, hir.ModuleDeclaration>(
  'getModuleDeclarationHir',
  provider: (context, moduleId) {
    final ast = context.callQuery(getModuleDeclarationAst, moduleId);
    return hir.ModuleDeclaration(
      parent: moduleId.parentOrNull,
      name: ast.name.name,
      innerDeclarationIds: context.callQuery(
        getInnerDeclarationIds,
        context.callQuery(moduleIdToDeclarationId, moduleId),
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
    final base =
        context.callQuery(resourceIdToModuleId, declarationId.resourceId);
    return base.nested(innerPath);
  },
);
final moduleIdToDeclarationId = Query<ModuleId, DeclarationId>(
  'moduleIdToDeclarationId',
  provider: (context, moduleId) {
    final declarationId =
        context.callQuery(moduleIdToOptionalDeclarationId, moduleId);
    assert(declarationId.isSome, 'Module `$moduleId` not found.');
    return declarationId.value;
  },
);
final moduleIdToOptionalDeclarationId = Query<ModuleId, Option<DeclarationId>>(
  'moduleIdToOptionalDeclarationId',
  provider: (context, rawModuleId) {
    final moduleId = rawModuleId.normalized;
    final packageId = moduleId.packageId;
    var path = '.';
    final remainingPath = moduleId.path.toList();

    assert(context.callQuery(
      doesResourceDirectoryExist,
      ResourceId(packageId, path),
    ));
    while (remainingPath.isNotEmpty &&
        context.callQuery(
          doesResourceDirectoryExist,
          ResourceId(packageId, '$path/${remainingPath.first}'),
        )) {
      // ignore: use_string_buffers
      path += '/${remainingPath.removeAt(0)}';
    }

    final moduleResourceId = ResourceId(packageId, '$path/$moduleFileName');
    ResourceId resourceId;
    if (context.callQuery(doesResourceExist, moduleResourceId)) {
      resourceId = moduleResourceId;
    } else {
      final fileResourceId = ResourceId(
        packageId,
        '$path/${remainingPath.first}$candyFileExtension',
      );
      if (remainingPath.isNotEmpty &&
          context.callQuery(doesResourceExist, fileResourceId)) {
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
    return context.callQuery(doesDeclarationExist, declarationId)
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

    final currentModuleId = context.callQuery(resourceIdToModuleId, resourceId);
    final relativeModuleId = currentModuleId.nested(modulePathSegments);
    final relativeDeclarationId =
        context.callQuery(moduleIdToOptionalDeclarationId, relativeModuleId);
    if (relativeDeclarationId.isSome) return relativeModuleId;

    final absoluteModuleId = ModuleId(
      PackageId(modulePathSegments.first),
      modulePathSegments.skip(1).toList(),
    );
    final absoluteDeclarationId =
        context.callQuery(moduleIdToOptionalDeclarationId, absoluteModuleId);
    if (absoluteDeclarationId.isSome) return absoluteModuleId;

    assert(false, "Use line `$inputs` couldn't be resolved.");
    return null;
  },
);
