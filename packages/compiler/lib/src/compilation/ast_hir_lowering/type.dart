import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

import '../../query.dart';
import '../../utils.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';
import 'declarations/declarations.dart';
import 'declarations/module.dart';

/// Resolves an AST-type in the given module to a HIR-type.
///
/// Nested union and intersection types (including intermediate group types) are
/// flattened.
final Query<Tuple2<ModuleId, ast.Type>, hir.CandyType> astTypeToHirType =
    Query<Tuple2<ModuleId, ast.Type>, hir.CandyType>(
  'astTypeToHirType',
  provider: (context, inputs) {
    final moduleId = inputs.first;
    final type = inputs.second;

    hir.CandyType map(ast.Type type) =>
        context.callQuery(astTypeToHirType, Tuple2(moduleId, type));
    List<hir.CandyType> mapTypes(Iterable<ast.Type> types) =>
        types.map(map).toList();

    if (type is ast.UserType) {
      final declarationId =
          context.callQuery(resolveAstUserType, Tuple2(moduleId, type));
      final name = declarationId.simplePath
          .lastWhile((d) => !(d is ModuleDeclarationPathData))
          .map((d) => (d as ModuleDeclarationPathData).name)
          .join('.');

      final arguments = mapTypes(type.arguments.arguments.map((a) => a.type));
      return hir.CandyType.user(
        context.callQuery(declarationIdToModuleId, declarationId),
        name,
        arguments: arguments,
      );
    } else if (type is ast.GroupType) {
      return context.callQuery(astTypeToHirType, type.type);
    } else if (type is ast.FunctionType) {
      return hir.CandyType.function(
        receiverType: type.receiver == null
            ? null
            : context.callQuery(astTypeToHirType, type.receiver),
        parameterTypes: mapTypes(type.parameterTypes),
        returnType: context.callQuery(astTypeToHirType, type.returnType),
      );
    } else if (type is ast.TupleType) {
      return hir.CandyType.tuple(mapTypes(type.types));
    } else if (type is ast.UnionType) {
      final leftResolved = map(type.leftType);
      final rightResolved = map(type.rightType);
      return hir.CandyType.union([
        if (leftResolved is hir.UnionCandyType)
          ...leftResolved.types
        else
          leftResolved,
        if (rightResolved is hir.UnionCandyType)
          ...rightResolved.types
        else
          rightResolved,
      ]);
    } else if (type is ast.IntersectionType) {
      final leftResolved = map(type.leftType);
      final rightResolved = map(type.rightType);
      return hir.CandyType.intersection([
        if (leftResolved is hir.IntersectionCandyType)
          ...leftResolved.types
        else
          leftResolved,
        if (rightResolved is hir.IntersectionCandyType)
          ...rightResolved.types
        else
          rightResolved,
      ]);
    }

    assert(false);
    return null;
  },
);

final resolveAstUserType = Query<Tuple2<ModuleId, ast.UserType>, DeclarationId>(
  'resolveAstUserType',
  provider: (context, inputs) {
    final currentModuleId = inputs.first;
    final type = inputs.second;

    final currentModuleDeclarationId =
        context.callQuery(moduleIdToDeclarationId, currentModuleId);
    final currentResourceId = currentModuleDeclarationId.resourceId;

    // Step 1: Look for traits/classes in outer modules in the same file.
    var moduleId = currentModuleId;
    var moduleDeclarationId = currentModuleDeclarationId;
    while (moduleDeclarationId.resourceId == currentResourceId) {
      final simpleTypes = type.simpleTypes.map((t) => t.name.name);
      var declarationId = moduleDeclarationId;
      for (final simpleType in simpleTypes) {
        declarationId =
            declarationId.inner(DeclarationPathData.trait(simpleType));
        if (context.callQuery(doesDeclarationExist, declarationId)) continue;

        // TODO(JonasWanke): traits in classes/enums allowed?
        declarationId =
            declarationId.inner(DeclarationPathData.class_(simpleType));
        if (context.callQuery(doesDeclarationExist, declarationId)) continue;

        break;
      }
      if (context.callQuery(doesDeclarationExist, declarationId)) {
        return declarationId;
      }

      if (moduleId.hasNoParent) break;
      moduleId = moduleId.parent;
      moduleDeclarationId =
          context.callQuery(moduleIdToDeclarationId, moduleId);
    }

    // TODO(JonasWank): Search imports.
    return null;
  },
);
