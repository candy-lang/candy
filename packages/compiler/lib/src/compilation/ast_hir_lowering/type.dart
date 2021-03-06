import 'package:parser/parser.dart' as ast;

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';
import 'declarations/class.dart';
import 'declarations/declarations.dart';
import 'declarations/function.dart';
import 'declarations/impl.dart';
import 'declarations/module.dart';
import 'declarations/property.dart';
import 'declarations/trait.dart';
import 'general.dart';

/// Resolves an AST-type in the given declaration (necessary to find type
/// parameters) to a HIR-type.
///
/// Nested union and intersection types (including intermediate group types) are
/// flattened.
// final Query<Tuple2<DeclarationId, ast.Type>, hir.CandyType> astTypeToHirType =
//     Query<Tuple2<DeclarationId, ast.Type>, hir.CandyType>(
//   'astTypeToHirType',
//   provider: (context, inputs) {
//     final declarationId = inputs.first;
//     final astType = inputs.second;
//     return lowerType(context, Tuple3(declarationId, second, third))
//   },
// );

final Query<Tuple2<DeclarationId, ast.Type>, hir.CandyType> astTypeToHirType =
    Query<Tuple2<DeclarationId, ast.Type>, hir.CandyType>(
  'astTypeToHirType',
  provider: (context, inputs) {
    final declarationId = inputs.first;
    final type = inputs.second;

    hir.CandyType map(ast.Type type) =>
        astTypeToHirType(context, Tuple2(declarationId, type));
    List<hir.CandyType> mapTypes(Iterable<ast.Type> types) =>
        types.map(map).toList();

    if (type is ast.UserType) {
      if (type.simpleTypes.length == 1 &&
          type.simpleTypes.single.name.name == 'This') {
        return hir.CandyType.this_();
      }

      final result = resolveAstUserType(context, Tuple2(declarationId, type));
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

final resolveAstUserType = Query<Tuple2<DeclarationId, ast.UserType>,
    hir.CandyType /*hir.UserCandyType | hir.ParameterCandyType */ >(
  'resolveAstUserType',
  provider: (context, inputs) {
    final declarationId = inputs.first;
    final type = inputs.second;

    // Step 1: Look for traits/classes in outer modules in the same file.
    final localResult = _resolveAstUserTypeInFile(context, declarationId, type);
    if (localResult.isSome) return localResult.value;

    // Step 2: Search use-lines.
    final resourceId = declarationId.resourceId;
    final simpleType = type.simpleTypes.first.name;
    final importedModuleResult = findModuleInUseLines(
      context,
      Tuple4(resourceId, simpleType.name, simpleType.span, false),
    );
    if (importedModuleResult is None) {
      throw CompilerError.typeNotFound(
        'Type `${simpleType.name}` could not be resolved.',
        location: ErrorLocation(resourceId, type.simpleTypes.first.span),
      );
    }
    final importedModule = importedModuleResult.value;
    final moduleId = importedModule
        .nested(type.simpleTypes.skip(1).map((it) => it.name.name).toList());

    final resultDeclarationId = moduleIdToDeclarationId(context, moduleId);
    if (resultDeclarationId.isModule) {
      throw CompilerError.typeNotFound(
        'Type `${simpleType.name}` could not be resolved.',
        location: ErrorLocation(resourceId, type.simpleTypes.first.span),
      );
    }
    assert(resultDeclarationId.isTrait || resultDeclarationId.isClass);
    return hir.CandyType.user(
      moduleId.parent,
      resultDeclarationId.simplePath.last.nameOrNull,
    );
  },
);

Option<hir.CandyType /*hir.UserCandyType | hir.ParameterCandyType */ >
    _resolveAstUserTypeInFile(
  QueryContext context,
  DeclarationId declarationId,
  ast.UserType type,
) {
  if (type.simpleTypes.length == 1) {
    final name = type.simpleTypes.first.name.name;
    final result =
        resolveAstUserTypeInParameters(context, Tuple2(declarationId, name));
    if (result.isSome) return result;
  }

  final resourceId = declarationId.resourceId;
  var currentModuleId = declarationIdToModuleId(context, declarationId);
  var currentModuleDeclarationId =
      moduleIdToDeclarationId(context, currentModuleId);
  while (currentModuleDeclarationId.resourceId == resourceId) {
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
            resourceId,
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
      return Option.some(hir.CandyType.user(currentModuleId, simpleTypes.last));
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

final resolveAstUserTypeInParameters =
    Query<Tuple2<DeclarationId, String>, Option<hir.ParameterCandyType>>(
  'resolveAstUserTypeInParameters',
  provider: (context, inputs) {
    final declarationId = inputs.first;
    final name = inputs.second;

    final astTypeParameters = <Tuple2<DeclarationId, ast.TypeParameters>>[];
    void addTypeParametersOf(DeclarationId id) {
      if (id.isTrait) {
        final traitAst = getTraitDeclarationAst(context, id);
        if (traitAst.typeParameters != null) {
          astTypeParameters.add(Tuple2(id, traitAst.typeParameters));
        }
      } else if (id.isImpl) {
        final implAst = getImplDeclarationAst(context, id);
        if (implAst.typeParameters != null) {
          astTypeParameters.add(Tuple2(id, implAst.typeParameters));
        }
      } else if (id.isClass) {
        final classAst = getClassDeclarationAst(context, id);
        if (classAst.typeParameters != null) {
          astTypeParameters.add(Tuple2(id, classAst.typeParameters));
        }
      } else if (id.isProperty) {
        final propertyAst = getPropertyDeclarationAst(context, id);
        if (!propertyAst.isStatic && id.hasParent) {
          addTypeParametersOf(id.parent);
        }
      } else if (id.isFunction) {
        final functionAst = getFunctionDeclarationAst(context, id);
        if (functionAst.typeParameters != null) {
          astTypeParameters.add(Tuple2(id, functionAst.typeParameters));
        }
        if (!functionAst.isStatic && !functionAst.isTest && id.hasParent) {
          addTypeParametersOf(id.parent);
        }
      }
    }

    final typeParameters = astTypeParameters.expand(
        (t) => t.second.parameters.map((p) => Tuple2(t.first, p.name.name)));

    addTypeParametersOf(declarationId);

    final matches = typeParameters.where((t) => t.second == name);
    if (matches.isNotEmpty) {
      return Some(hir.ParameterCandyType(name, matches.first.first));
    }
    return None();
  },
);
