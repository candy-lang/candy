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
}

final getModuleDeclarationAst = Query<ModuleId, ast.ModuleDeclaration>(
  'getModuleDeclarationAst',
  provider: (context, moduleId) {
    final declarationId = context.callQuery(moduleIdToDeclarationId, moduleId);
    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration != null, 'Module $moduleId not found.');
    assert(declaration is ast.ModuleDeclaration, 'Wrong return type.');
    return declaration as ast.ModuleDeclaration;
  },
);
final getModuleDeclarationHir = Query<ModuleId, hir.ModuleDeclaration>(
  'getModuleDeclarationHir',
  provider: (context, moduleId) {
    final declarationId = context.callQuery(moduleIdToDeclarationId, moduleId);
    assert(declarationId != null);

    final ast = context.callQuery(getModuleDeclarationAst, declarationId);
    return hir.ModuleDeclaration(
      parent: moduleId.parent,
      name: ast.name.name,
      // TODO(JonasWanke): child declarations
    );
  },
);

final declarationIdToModuleId = Query<DeclarationId, ModuleId>(
  'declarationIdToModuleId',
  evaluateAlways: true,
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
  provider: (context, rawModuleId) {
    final moduleId = rawModuleId.normalized;
    final packageId = moduleId.packageId;
    var path = '.';
    final remainingPath = moduleId.path.toList();

    while (context.callQuery(
      doesResourceDirectoryExist,
      ResourceId(packageId, path),
    )) {
      // ignore: use_string_buffers
      path += '/${remainingPath.removeAt(0)}';
    }

    final fileResourceId = ResourceId(
      packageId,
      '$path/${remainingPath.first}$candyFileExtension',
    );
    final moduleResourceId = ResourceId(
      packageId,
      '$path/$moduleFileName',
    );
    ResourceId resourceId;
    if (context.callQuery(doesResourceExist, fileResourceId)) {
      resourceId = fileResourceId;
    } else {
      assert(context.callQuery(doesResourceExist, moduleResourceId));
      resourceId = moduleResourceId;
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
        ? declarationId
        : null;
  },
);
final resourceIdToModuleId = Query<ResourceId, ModuleId>(
  'resourceIdToModuleId',
  evaluateAlways: true,
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
        context.callQuery(moduleIdToDeclarationId, relativeModuleId);
    if (relativeDeclarationId != null) return relativeModuleId;

    final absoluteModuleId = ModuleId(
      PackageId(modulePathSegments.first),
      modulePathSegments.skip(1).toList(),
    );
    final absoluteDeclarationId =
        context.callQuery(moduleIdToDeclarationId, absoluteModuleId);
    if (absoluteDeclarationId != null) return absoluteModuleId;

    return null;
  },
);
