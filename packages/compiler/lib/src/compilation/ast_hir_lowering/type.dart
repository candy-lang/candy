import 'package:parser/parser.dart' as ast;

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';
import 'declarations/class.dart';
import 'declarations/declarations.dart';
import 'declarations/impl.dart';
import 'declarations/module.dart';
import 'declarations/trait.dart';
import 'general.dart';

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
        astTypeToHirType(context, Tuple2(moduleId, type));
    List<hir.CandyType> mapTypes(Iterable<ast.Type> types) =>
        types.map(map).toList();

    if (type is ast.UserType) {
      final result = resolveAstUserType(context, Tuple2(moduleId, type));
      if (result is! hir.UserCandyType) return result;

      return (result as hir.UserCandyType).copyWith(
        arguments:
            mapTypes((type.arguments?.arguments ?? []).map((a) => a.type)),
      );
    } else if (type is ast.GroupType) {
      return map(type.type);
    } else if (type is ast.FunctionType) {
      return hir.CandyType.function(
        receiverType: type.receiver == null ? null : map(type.receiver),
        parameterTypes: mapTypes(type.parameterTypes),
        returnType: map(type.returnType),
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

final resolveAstUserType = Query<Tuple2<ModuleId, ast.UserType>,
    hir.CandyType /*hir.UserCandyType | hir.ParameterCandyType */ >(
  'resolveAstUserType',
  provider: (context, inputs) {
    final moduleId = inputs.first;
    final type = inputs.second;

    if (type.simpleTypes.length > 1) {
      throw CompilerError.unsupportedFeature(
        'Nested types are not yet supported.',
      );
    }

    // Step 1: Look for traits/classes in outer modules in the same file.
    final localResult = _resolveAstUserTypeInFile(context, moduleId, type);
    if (localResult.isSome) return localResult.value;

    // Step 2: Search use-lines.
    final resourceId = moduleIdToDeclarationId(context, moduleId).resourceId;
    final simpleType = type.simpleTypes.first.name;
    final importedModules = findModuleInUseLines(
      context,
      Tuple4(resourceId, simpleType.name, simpleType.span, false),
    );
    if (importedModules is None) {
      throw CompilerError.typeNotFound(
        'Type `${simpleType.name}` could not be resolved.',
        location: ErrorLocation(resourceId, type.simpleTypes.first.span),
      );
    }

    final declarationId =
        moduleIdToDeclarationId(context, importedModules.value);
    if (declarationId.isModule) {
      throw CompilerError.typeNotFound(
        'Type `${simpleType.name}` could not be resolved.',
        location: ErrorLocation(resourceId, type.simpleTypes.first.span),
      );
    }
    assert(declarationId.isTrait || declarationId.isClass);
    return hir.CandyType.user(
      importedModules.value.parent,
      declarationId.simplePath.last.nameOrNull,
    );
  },
);

Option<hir.CandyType /*hir.UserCandyType | hir.ParameterCandyType */ >
    _resolveAstUserTypeInFile(
  QueryContext context,
  ModuleId moduleId,
  ast.UserType type,
) {
  final moduleDeclarationId = moduleIdToDeclarationId(context, moduleId);
  if (type.simpleTypes.length == 1) {
    final name = type.simpleTypes.single.name.name;

    List<hir.TypeParameter> typeParameters;
    if (moduleDeclarationId.isTrait) {
      final traitHir = getTraitDeclarationHir(context, moduleDeclarationId);
      typeParameters = traitHir.typeParameters;
    } else if (moduleDeclarationId.isImpl) {
      final implHir = getImplDeclarationHir(context, moduleDeclarationId);
      typeParameters = implHir.typeParameters;
    } else if (moduleDeclarationId.isClass) {
      final classHir = getClassDeclarationHir(context, moduleDeclarationId);
      typeParameters = classHir.typeParameters;
    }

    if (typeParameters != null) {
      final matches = typeParameters.where((p) => p.name == name);
      assert(matches.length <= 1);

      if (matches.isNotEmpty) {
        return Some(hir.CandyType.parameter(name, moduleDeclarationId));
      }
    }
  }

  var currentModuleId = moduleId;
  var currentModuleDeclarationId = moduleDeclarationId;
  while (
      currentModuleDeclarationId.resourceId == moduleDeclarationId.resourceId) {
    final simpleTypes = type.simpleTypes.map((t) => t.name.name);
    var declarationId = currentModuleDeclarationId;
    for (final simpleType in simpleTypes) {
      final traitId =
          declarationId.inner(DeclarationPathData.trait(simpleType));
      final hasTrait = doesDeclarationExist(context, traitId);

      // TODO(JonasWanke): traits in classes/enums allowed?
      final classId =
          declarationId.inner(DeclarationPathData.class_(simpleType));
      final hasClass = doesDeclarationExist(context, classId);

      if (hasTrait && hasClass) {
        context.reportError(CompilerError.multipleTypesWithSameName(
          'Multiple types have the same name: `$simpleType` in module `$currentModuleId`.',
          location: ErrorLocation(
            moduleDeclarationId.resourceId,
            getTraitDeclarationAst(context, traitId).name.span,
          ),
        ));
      }
      if (!hasTrait && !hasClass) {
        declarationId = null;
        break;
      }

      declarationId = hasTrait ? traitId : classId;
    }
    if (declarationId != null) {
      return Option.some(hir.CandyType.user(
        currentModuleId.parent,
        simpleTypes.last,
      ));
    }

    if (currentModuleId.hasNoParent) break;
    currentModuleId = currentModuleId.parentOrNull;
    final newDeclarationId =
        moduleIdToOptionalDeclarationId(context, currentModuleId);
    if (newDeclarationId is None) break;
    currentModuleDeclarationId = newDeclarationId.value;
  }
  return Option.none();
}
