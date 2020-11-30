import 'package:compiler/src/compilation/hir.dart';
import 'package:dartx/dartx.dart';
import 'package:freezed_annotation/freezed_annotation.dart';

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast_hir_lowering.dart';
import '../ast_hir_lowering/declarations/impl.dart';
import '../ids.dart';
import 'declarations.dart';
import 'ids.dart';

part 'type.freezed.dart';
part 'type.g.dart';

// ignore_for_file: sort_constructors_first

@freezed
abstract class CandyType with _$CandyType {
  const factory CandyType.user(
    ModuleId parentModuleId,
    String name, {
    @Default(<CandyType>[]) List<CandyType> arguments,
  }) = UserCandyType;
  // ignore: non_constant_identifier_names
  const factory CandyType.this_() = ThisCandyType;
  const factory CandyType.tuple(List<CandyType> items) = TupleCandyType;
  const factory CandyType.function({
    CandyType receiverType,
    @Default(<CandyType>[]) List<CandyType> parameterTypes,
    @required CandyType returnType,
  }) = FunctionCandyType;
  @Assert('types.length > 0')
  const factory CandyType.union(List<CandyType> types) = UnionCandyType;
  @Assert('types.length > 0')
  const factory CandyType.intersection(List<CandyType> types) =
      IntersectionCandyType;
  const factory CandyType.parameter(String name, DeclarationId declarationId) =
      ParameterCandyType;
  const factory CandyType.meta(CandyType baseType) = MetaCandyType;
  const factory CandyType.reflection(DeclarationId declarationId) =
      ReflectionCandyType;

  factory CandyType.fromJson(Map<String, dynamic> json) =>
      _$CandyTypeFromJson(json);
  const CandyType._();

  // primitives types
  static const any = CandyType.user(ModuleId.corePrimitives, 'Any');
  static const unit = CandyType.user(ModuleId.corePrimitives, 'Unit');
  static const never = CandyType.user(ModuleId.corePrimitives, 'Never');

  // other important types
  static const bool = CandyType.user(ModuleId.coreBool, 'Bool');
  static const number = CandyType.user(ModuleId.coreNumbers, 'Number');
  static const int = CandyType.user(ModuleId.coreNumbersInt, 'Int');
  static const float = CandyType.user(ModuleId.coreNumbersFloat, 'Float');
  static const string = CandyType.user(ModuleId.coreString, 'String');

  factory CandyType.maybe(CandyType itemType) => CandyType.user(
        ModuleId.coreMaybe,
        'Maybe',
        arguments: [itemType],
      );
  factory CandyType.some(CandyType itemType) => CandyType.user(
        ModuleId.coreMaybe,
        'Some',
        arguments: [itemType],
      );

  // collections
  factory CandyType.iterator(CandyType itemType) => CandyType.user(
        ModuleId.coreCollections.nested(['iterable']),
        'Iterator',
        arguments: [itemType],
      );
  static const iterableModuleId =
      ModuleId(PackageId.core, ['collections', 'iterable', 'Iterable']);
  factory CandyType.iterable(CandyType itemType) => CandyType.user(
        ModuleId.coreCollections.nested(['iterable']),
        'Iterable',
        arguments: [itemType],
      );
  static const listModuleId =
      ModuleId(PackageId.core, ['collections', 'list', 'List']);
  factory CandyType.list(CandyType itemType) => CandyType.user(
        ModuleId.coreCollections.nested(['list']),
        'List',
        arguments: [itemType],
      );
  factory CandyType.arrayList(CandyType itemType) => CandyType.user(
        ModuleId.coreCollections.nested(['list', 'array_list']),
        'ArrayList',
        arguments: [itemType],
      );
  static const arrayListModuleId = ModuleId(
      PackageId.core, ['collections', 'list', 'array_list', 'ArrayList']);
  static const arrayModuleId =
      ModuleId(PackageId.core, ['collections', 'array', 'Array']);

  // operators
  // operators.arithmetic
  static final add = UserCandyType(ModuleId.coreOperatorsArithmetic, 'Add');
  static final subtract =
      UserCandyType(ModuleId.coreOperatorsArithmetic, 'Subtract');
  static final negate =
      UserCandyType(ModuleId.coreOperatorsArithmetic, 'Negate');
  static final multiply =
      UserCandyType(ModuleId.coreOperatorsArithmetic, 'Multiply');
  static final divide =
      UserCandyType(ModuleId.coreOperatorsArithmetic, 'Divide');
  static final divideTruncating =
      UserCandyType(ModuleId.coreOperatorsArithmetic, 'DivideTruncating');
  static final modulo =
      UserCandyType(ModuleId.coreOperatorsArithmetic, 'Modulo');
  // operators.comparison
  static final comparable =
      UserCandyType(ModuleId.coreOperatorsComparison, 'Comparable');
  // operators.equality
  static final equals = UserCandyType(ModuleId.coreOperatorsEquality, 'Equals');
  // operators.logical
  static const and = UserCandyType(ModuleId.coreOperatorsLogical, 'And');
  static const or = UserCandyType(ModuleId.coreOperatorsLogical, 'Or');
  static const opposite =
      UserCandyType(ModuleId.coreOperatorsLogical, 'Opposite');
  static const implies =
      UserCandyType(ModuleId.coreOperatorsLogical, 'Implies');

  // random
  static const randomSource =
      UserCandyType(ModuleId.coreRandomSource, 'RandomSource');

  // reflection
  static const type = UserCandyType(ModuleId.coreReflection, 'Type');
  static const module = UserCandyType(ModuleId.coreReflection, 'Module');

  ModuleId get virtualModuleId => maybeWhen(
        user: (moduleId, name, _) => moduleId.nested([name]),
        orElse: () {
          throw CompilerError.internalError(
            '`virtualModuleId` called on non-user type `$runtimeType`.',
          );
        },
      );

  CandyType bakeThisType(CandyType thisType) {
    if (thisType == null) return this;

    return map(
      user: (type) => type.copyWith(
          arguments:
              type.arguments.map((a) => a.bakeThisType(thisType)).toList()),
      this_: (_) => thisType,
      tuple: (type) => type.copyWith(
          items: type.items.map((i) => i.bakeThisType(thisType)).toList()),
      function: (type) => type.copyWith(
        receiverType: type.receiverType?.bakeThisType(thisType),
        parameterTypes:
            type.parameterTypes.map((p) => p.bakeThisType(thisType)).toList(),
        returnType: type.returnType.bakeThisType(thisType),
      ),
      union: (type) => type.copyWith(
          types: type.types.map((t) => t.bakeThisType(thisType)).toList()),
      intersection: (type) => type.copyWith(
          types: type.types.map((t) => t.bakeThisType(thisType)).toList()),
      parameter: (type) => type,
      meta: (type) => type,
      reflection: (type) => type,
    );
  }

  CandyType bakeGenerics(Map<CandyType, CandyType> types) {
    if (types.isEmpty) return this;

    return map(
      user: (type) => type.copyWith(
          arguments: type.arguments.map((a) => a.bakeGenerics(types)).toList()),
      this_: (type) => type,
      tuple: (type) => type.copyWith(
          items: type.items.map((i) => i.bakeGenerics(types)).toList()),
      function: (type) => type.copyWith(
        receiverType: type.receiverType?.bakeGenerics(types),
        parameterTypes:
            type.parameterTypes.map((p) => p.bakeGenerics(types)).toList(),
        returnType: type.returnType.bakeGenerics(types),
      ),
      union: (type) => type.copyWith(
          types: type.types.map((t) => t.bakeGenerics(types)).toList()),
      intersection: (type) => type.copyWith(
          types: type.types.map((t) => t.bakeGenerics(types)).toList()),
      parameter: (type) => types[type] ?? type,
      meta: (type) => type,
      reflection: (type) => type,
    );
  }

  @override
  String toString() {
    return map(
      user: (type) {
        var name = '${type.parentModuleId}:${type.name}';
        if (type.arguments.isNotEmpty) name += '<${type.arguments.join(', ')}>';
        return name;
      },
      this_: (_) => 'This',
      tuple: (type) => '(${type.items.join(', ')})',
      function: (type) {
        var name = '(${type.parameterTypes.join(', ')}) => ${type.returnType}';
        if (type.receiverType != null) name = '${type.receiverType}.$name';
        return name;
      },
      union: (type) => type.types.join(' | '),
      intersection: (type) => type.types.join(' & '),
      parameter: (type) => '${type.name}@${type.declarationId}',
      meta: (type) {
        final base = type.baseType;
        if (base is UserCandyType) return 'Type<${base.virtualModuleId}>';
        if (base is ParameterCandyType) return 'Type<$base>';
        throw CompilerError.internalError(
          'Invalid meta target in `CandyType.toString()`: `$base`.',
        );
      },
      reflection: (type) {
        final id = type.declarationId;
        if (id.isModule) return 'Module<$id>';
        if (id.isFunction) return 'Function<$id>';
        if (id.isProperty) return 'Property<$id>';
        throw CompilerError.internalError(
          'Invalid reflection target in `CandyType.toString()`: `$id`.',
        );
      },
    );
  }
}

final getTypeParameterBound = Query<ParameterCandyType, CandyType>(
  'getTypeParameterBound',
  provider: (context, parameter) {
    List<TypeParameter> parameters;
    if (parameter.declarationId.isTrait) {
      parameters = getTraitDeclarationHir(context, parameter.declarationId)
          .typeParameters;
    } else if (parameter.declarationId.isImpl) {
      parameters = getImplDeclarationHir(context, parameter.declarationId)
          .typeParameters;
    } else if (parameter.declarationId.isClass) {
      parameters = getClassDeclarationHir(context, parameter.declarationId)
          .typeParameters;
    } else if (parameter.declarationId.isFunction) {
      parameters = getFunctionDeclarationHir(context, parameter.declarationId)
          .typeParameters;
    } else {
      throw CompilerError.internalError(
        'Type parameter comes from neither a trait, nor an impl, class, or a function.',
      );
    }

    return parameters.singleWhere((p) => p.name == parameter.name).upperBound;
  },
);

final Query<Tuple2<CandyType, CandyType>, bool> isAssignableTo =
    Query<Tuple2<CandyType, CandyType>, bool>(
  'isAssignableTo',
  provider: (context, inputs) {
    final child = inputs.first;
    final parent = inputs.second;

    if (child == parent) return true;
    if (parent == CandyType.any) return true;
    if (child == CandyType.any) return false;
    if (child == CandyType.never) return true;
    if (parent == CandyType.never) return false;

    ReportedCompilerError invalidThisType() {
      return CompilerError.internalError(
        '`isAssignableTo` was called with an invalid `This`-type.',
      );
    }

    // TODO(marcelgarus): This is ugly and hardcoded.
    if (child is UserCandyType && parent is UserCandyType) {
      if (child.name == 'None' && parent.name == 'Option') {
        return true;
      } else if (child.name == 'Some' && parent.name == 'Option') {
        return child.arguments
            .zip(parent.arguments,
                (a, CandyType b) => isAssignableTo(context, Tuple2(a, b)))
            .every((it) => it);
      }
    }

    CandyType getResultingType(ReflectionCandyType type) {
      final id = type.declarationId;
      if (id.isModule) return CandyType.module;

      if (id.isFunction) {
        final functionHir = getFunctionDeclarationHir(context, id);

        return functionHir.functionType.copyWith(
          receiverType:
              getPropertyDeclarationParentAsType(context, id).valueOrNull,
        );
      }
      if (id.isProperty) {
        final propertyHir = getPropertyDeclarationHir(context, id);
        if (propertyHir.isStatic) return propertyHir.type;

        return FunctionCandyType(
          receiverType:
              getPropertyDeclarationParentAsType(context, id).valueOrNull,
          returnType: propertyHir.type,
        );
      }
      throw CompilerError.internalError('Invalid reflection target: `$id`.');
    }

    return child.map(
      user: (childType) {
        return parent.map(
          user: (parentType) {
            final declarationId =
                moduleIdToDeclarationId(context, childType.virtualModuleId);
            if (declarationId.isTrait) {
              final declaration =
                  getTraitDeclarationHir(context, declarationId);

              return declaration.upperBounds.any((bound) {
                final bakedBound = bound.bakeGenerics({
                  for (final index in childType.arguments.indices)
                    CandyType.parameter(declaration.typeParameters[index].name,
                        declarationId): childType.arguments[index],
                });
                return isAssignableTo(
                  context,
                  Tuple2(bakedBound, parent),
                );
              });
            }

            if (declarationId.isClass) {
              if (parent is! UserCandyType) return false;

              return getClassTraitImplId(
                context,
                Tuple2(childType, parentType),
              ) is Some;
            }

            throw CompilerError.internalError(
              'User type can only be a trait or a class.',
            );
          },
          this_: (_) => throw invalidThisType(),
          tuple: (_) => false,
          function: (_) => false,
          union: (parentType) => parentType.types
              .any((type) => isAssignableTo(context, Tuple2(childType, type))),
          intersection: (parentType) => parentType.types.every(
              (type) => isAssignableTo(context, Tuple2(childType, type))),
          parameter: (type) => false,
          meta: (_) => isAssignableTo(context, Tuple2(child, CandyType.type)),
          reflection: (type) => isAssignableTo(
            context,
            Tuple2(getResultingType(type), parent),
          ),
        );
      },
      this_: (_) => throw invalidThisType(),
      tuple: (type) {
        if (parent is TupleCandyType) {
          return type.items.length == parent.items.length &&
              type.items
                  .zip<CandyType, bool>(parent.items,
                      (a, b) => isAssignableTo(context, Tuple2(a, b)))
                  .every((it) => it);
        }
        return false;
      },
      function: (type) {
        throw CompilerError.unsupportedFeature(
          'Trait implementations for functions are not yet supported.',
        );
      },
      union: (type) {
        final items = type.types;
        assert(items.length >= 2);
        return items
            .every((type) => isAssignableTo(context, Tuple2(type, parent)));
      },
      intersection: (type) {
        final items = type.types;
        assert(items.length >= 2);
        return items
            .any((type) => isAssignableTo(context, Tuple2(type, parent)));
      },
      parameter: (type) {
        final bound = getTypeParameterBound(context, type);
        return isAssignableTo(context, Tuple2(bound, parent));
      },
      meta: (_) => isAssignableTo(context, Tuple2(CandyType.type, parent)),
      reflection: (type) =>
          isAssignableTo(context, Tuple2(getResultingType(type), parent)),
    );
  },
);

final getClassTraitImplId =
    Query<Tuple2<UserCandyType, UserCandyType>, Option<DeclarationId>>(
  'getClassTraitImplId',
  provider: (context, inputs) {
    final child = inputs.first;
    final parent = inputs.second;

    final implIds = getAllImplsForType(context, child).where((implId) {
      final impl = getImplDeclarationHir(context, implId);
      // TODO(marcelgarus): Constraints solving should go here.
      return impl.traits.any((trait) => trait.name == parent.name);
    });
    if (implIds.length > 1) {
      throw CompilerError.ambiguousImplsFound(
        'Multiple impls found for class `$child` and trait `$parent`.',
        location: ErrorLocation(
          implIds.first.resourceId,
          getImplDeclarationAst(context, implIds.first).representativeSpan,
        ),
        relatedInformation: [
          for (final implId in implIds)
            ErrorRelatedInformation(
              location: ErrorLocation(
                implIds.first.resourceId,
                getImplDeclarationAst(context, implId).representativeSpan,
              ),
              message: 'An impl is here.',
            ),
        ],
      );
    }

    if (implIds.isEmpty) return None();
    return Some(implIds.single);
  },
);

final getPropertyDeclarationParentAsType =
    Query<DeclarationId, Option<UserCandyType>>(
  'getPropertyDeclarationParentAsType',
  provider: (context, declarationId) {
    final parentId = declarationId.parent;
    if (parentId.isTrait) {
      final parentHir = getTraitDeclarationHir(context, parentId);
      if (parentHir.typeParameters.isNotEmpty) {
        throw CompilerError.unsupportedFeature(
          'References to instance methods of traits with type parameters are not yet supported.',
        );
      }
      return Some(UserCandyType(
        declarationIdToModuleId(context, parentId.parent),
        parentHir.name,
      ));
    } else if (parentId.isImpl) {
      final parentHir = getImplDeclarationHir(context, parentId);
      if (parentHir.typeParameters.isNotEmpty ||
          parentHir.type.arguments.isNotEmpty) {
        throw CompilerError.unsupportedFeature(
          'References to instance methods of impls with type parameters (or for a type with type arguments) are not yet supported.',
        );
      }
      return Some(UserCandyType(
        declarationIdToModuleId(context, parentId.parent),
        parentHir.type.name,
      ));
    } else if (parentId.isClass) {
      final parentHir = getClassDeclarationHir(context, parentId);
      if (parentHir.typeParameters.isNotEmpty) {
        throw CompilerError.unsupportedFeature(
          'References to instance methods of classes with type parameters are not yet supported.',
        );
      }
      return Some(UserCandyType(
        declarationIdToModuleId(context, parentId.parent),
        parentHir.name,
      ));
    } else {
      assert(parentId.isModule);
      return None();
    }
  },
);
