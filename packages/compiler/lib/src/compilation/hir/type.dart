import 'package:freezed_annotation/freezed_annotation.dart';

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast_hir_lowering.dart';
import 'ids.dart';

part 'type.freezed.dart';
part 'type.g.dart';

// ignore_for_file: sort_constructors_first

@freezed
abstract class CandyType with _$CandyType {
  const factory CandyType.user(
    ModuleId moduleId,
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
  static const nothing = CandyType.user(ModuleId.corePrimitives, 'Nothing');

  static const bool = CandyType.user(ModuleId.corePrimitives, 'Bool');
  static const number = CandyType.user(ModuleId.corePrimitives, 'Number');
  static const int = CandyType.user(ModuleId.corePrimitives, 'Int');
  static const float = CandyType.user(ModuleId.corePrimitives, 'Float');
  static const string = CandyType.user(ModuleId.corePrimitives, 'String');

  factory CandyType.list(CandyType itemType) =>
      CandyType.user(ModuleId.coreCollections, 'List', arguments: [itemType]);
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

    return child.when(
      user: (moduleId, name, _) {
        final moduleDeclarationId = moduleIdToDeclarationId(context, moduleId);
        final traitId =
            moduleDeclarationId.inner(TraitDeclarationPathData(name));
        if (doesDeclarationExist(context, traitId)) {
          final declaration = getTraitDeclarationHir(context, traitId);
          if (declaration.typeParameters.isNotEmpty) {
            throw CompilerError.unsupportedFeature(
              'Type parameters are not yet supported.',
            );
          }
          return isAssignableTo(
            context,
            Tuple2(declaration.upperBound, parent),
          );
        }

        throw CompilerError.unsupportedFeature(
          'Trait implementations for classes are not yet supported.',
        );
      },
      tuple: (items) {
        throw CompilerError.unsupportedFeature(
          'Trait implementations for tuples are not yet supported.',
        );
      },
      function: (receiverType, parameterTypes, returnType) {
        throw CompilerError.unsupportedFeature(
          'Trait implementations for functions are not yet supported.',
        );
      },
      union: (items) {
        assert(items.length >= 2);
        return items
            .every((type) => isAssignableTo(context, Tuple2(type, parent)));
      },
      intersection: (items) {
        assert(items.length >= 2);
        return items
            .any((type) => isAssignableTo(context, Tuple2(type, parent)));
      },
    );
  },
);
