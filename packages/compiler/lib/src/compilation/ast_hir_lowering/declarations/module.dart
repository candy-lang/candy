import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

import '../../../constants.dart';
import '../../../errors.dart';
import '../../../query.dart';
import '../../../utils.dart';
import '../../ast/parser.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
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
    final innerPath = declarationId.simplePath.mapNotNull((pathData) {
      if (pathData is ModuleDeclarationPathData) return pathData.name;
      if (pathData is TraitDeclarationPathData) return pathData.name;
      if (pathData is ClassDeclarationPathData) return pathData.name;
      return null;
    }).toList();
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
  provider: (context, moduleId) {
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

    ResourceId resourceId;
    if (remainingPath.isNotEmpty) {
      final fileResourceId = ResourceId(
        packageId,
        pathAnd('${remainingPath.first}$candyFileExtension'),
      );
      if (doesResourceExist(context, fileResourceId)) {
        resourceId = resourceId = fileResourceId;
        remainingPath.removeAt(0);
      }
    }
    if (resourceId == null) {
      final moduleResourceId = ResourceId(packageId, pathAnd(moduleFileName));
      if (doesResourceExist(context, moduleResourceId)) {
        resourceId = moduleResourceId;
      }
    }
    if (resourceId == null) {
      return Option.none();
    }
    assert(doesResourceExist(context, resourceId));

    var declarationId = DeclarationId(resourceId);
    while (remainingPath.isNotEmpty) {
      final name = remainingPath.first;

      final moduleId = declarationId.inner(DeclarationPathData.module(name));
      if (doesDeclarationExist(context, moduleId)) {
        declarationId = moduleId;
        remainingPath.removeAt(0);
        continue;
      }
      final traitId = declarationId.inner(DeclarationPathData.trait(name));
      if (doesDeclarationExist(context, traitId)) {
        declarationId = traitId;
        remainingPath.removeAt(0);
        continue;
      }
      final classId = declarationId.inner(DeclarationPathData.class_(name));
      if (doesDeclarationExist(context, classId)) {
        declarationId = classId;
        remainingPath.removeAt(0);
        continue;
      }

      return Option.none();
    }
    return Option.some(declarationId);
  },
);
final resourceIdToModuleId = Query<ResourceId, ModuleId>(
  'resourceIdToModuleId',
  persist: false,
  provider: (context, resourceId) {
    assert(resourceId.isCandyFile);

    var path = resourceId.path.removeSuffix(candyFileExtension).split('/');
    if (resourceId.isModuleFile) path = path.dropLast(1);
    return ModuleId(resourceId.packageId, path);
  },
);
