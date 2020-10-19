import 'package:freezed_annotation/freezed_annotation.dart';

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast_hir_lowering.dart';
import '../ast_hir_lowering/declarations/impl.dart';
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
  const factory CandyType.tuple(List<CandyType> items) = TupleCandyType;
  const factory CandyType.function({
    CandyType receiverType,
    @Default(<CandyType>[]) List<CandyType> parameterTypes,
    @required CandyType returnType,
  }) = FunctionCandyType;
  const factory CandyType.union(List<CandyType> types) = UnionCandyType;
  const factory CandyType.intersection(List<CandyType> types) =
      IntersectionCandyType;

  factory CandyType.fromJson(Map<String, dynamic> json) =>
      _$CandyTypeFromJson(json);
  const CandyType._();

  static const any = CandyType.user(ModuleId.corePrimitives, 'Any');
  static const unit = CandyType.user(ModuleId.corePrimitives, 'Unit');
  static const never = CandyType.user(ModuleId.corePrimitives, 'Never');

  static const bool = CandyType.user(ModuleId.corePrimitives, 'Bool');
  static const number = CandyType.user(ModuleId.corePrimitives, 'Number');
  static const int = CandyType.user(ModuleId.corePrimitives, 'Int');
  static const float = CandyType.user(ModuleId.corePrimitives, 'Float');
  static const string = CandyType.user(ModuleId.corePrimitives, 'String');

  static const declaration =
      CandyType.user(ModuleId.coreReflection, 'Declaration');
  static const moduleDeclaration =
      CandyType.user(ModuleId.coreReflection, 'ModuleDeclaration');

  factory CandyType.list(CandyType itemType) =>
      CandyType.user(ModuleId.coreCollections, 'List', arguments: [itemType]);

  ModuleId get virtualModuleId => maybeWhen(
        user: (moduleId, name, _) => moduleId.nested([name]),
        orElse: () {
          throw CompilerError.internalError(
            '`virtualModuleId` called on non-user type `$runtimeType`.',
          );
        },
      );

  @override
  String toString() {
    return map(
      user: (type) {
        var name = '${type.parentModuleId}:${type.name}';
        if (type.arguments.isNotEmpty) name += '<${type.arguments.join(', ')}>';
        return name;
      },
      tuple: (type) => '(${type.items.join(', ')})',
      function: (type) {
        var name = '(${type.parameterTypes.join(', ')}) => ${type.returnType}';
        if (type.receiverType != null) name = '${type.receiverType}.$name';
        return name;
      },
      union: (type) => type.types.join(' | '),
      intersection: (type) => type.types.join(' & '),
    );
  }
}

final Query<Tuple2<CandyType, CandyType>, bool> isAssignableTo =
    Query<Tuple2<CandyType, CandyType>, bool>(
  'isAssignableTo',
  provider: (context, inputs) {
    final child = inputs.first;
    final parent = inputs.second;

    if (child == parent) return true;
    if (parent == CandyType.any) return true;
    if (child == CandyType.any) return false;

    return child.map(
      user: (childType) {
        return parent.map(
          user: (parentType) {
            final declarationId =
                moduleIdToDeclarationId(context, childType.virtualModuleId);
            if (declarationId.isTrait) {
              final declaration =
                  getTraitDeclarationHir(context, declarationId);
              if (declaration.typeParameters.isNotEmpty) {
                throw CompilerError.unsupportedFeature(
                  'Type parameters are not yet supported.',
                );
              }
              return declaration.upperBounds.any(
                  (bound) => isAssignableTo(context, Tuple2(bound, parent)));
            }

            if (declarationId.isClass) {
              if (parent is! UserCandyType) return false;

              return getClassTraitImplId(context, inputs) is Some;
            }

            throw CompilerError.internalError(
              'User type can only be a trait or a class.',
            );
          },
          tuple: (_) => false,
          function: (_) => false,
          union: (parentType) => parentType.types
              .any((type) => isAssignableTo(context, Tuple2(childType, type))),
          intersection: (parentType) => parentType.types.every(
              (type) => isAssignableTo(context, Tuple2(childType, type))),
        );
      },
      tuple: (type) {
        throw CompilerError.unsupportedFeature(
          'Trait implementations for tuples are not yet supported.',
        );
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
    );
  },
);

final getClassTraitImplId =
    Query<Tuple2<CandyType, CandyType>, Option<DeclarationId>>(
  'getClassTraitImplId',
  provider: (context, inputs) {
    assert(inputs.first is UserCandyType);
    final child = inputs.first as UserCandyType;
    assert(inputs.second is UserCandyType);
    final parent = inputs.second as UserCandyType;

    final implIds = {
      child.parentModuleId.packageId,
      parent.parentModuleId.packageId,
    }
        .expand((packageId) =>
            getAllImplsForType(context, Tuple2(child, packageId)))
        .where((implId) {
      final impl = getImplDeclarationHir(context, implId);
      return impl.traits.any((trait) => trait == parent);
    });
    if (implIds.length > 1) {
      throw CompilerError.ambiguousImplsFound(
        'Multiple impls found for class `$child` and trait `$parent`.',
        location: ErrorLocation(
          implIds.first.resourceId,
          getImplDeclarationAst(context, implIds.first).representativeSpan,
        ),
        // TODO(JonasWanke): output other impl locations
      );
    }

    if (implIds.isEmpty) return None();
    return Some(implIds.single);
  },
);
