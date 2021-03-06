import 'package:freezed_annotation/freezed_annotation.dart';

import '../../utils.dart';
import 'expressions.dart';
import 'ids.dart';
import 'type.dart';

part 'declarations.freezed.dart';

@freezed
abstract class Declaration implements _$Declaration {
  const factory Declaration.module(
    DeclarationId id, {
    ModuleId parent,
    @required String name,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = ModuleDeclaration;

  const factory Declaration.trait(
    DeclarationId id,
    String name, {
    @required UserCandyType thisType,
    @Default(<TypeParameter>[]) List<TypeParameter> typeParameters,
    @required List<UserCandyType> upperBounds,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = TraitDeclaration;

  const factory Declaration.impl(
    DeclarationId id, {
    @Default(<TypeParameter>[]) List<TypeParameter> typeParameters,
    @required UserCandyType type,
    @required List<UserCandyType> traits,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = ImplDeclaration;

  // ignore: non_constant_identifier_names
  const factory Declaration.class_(
    DeclarationId id, {
    @required String name,
    @required UserCandyType thisType,
    @Default(<TypeParameter>[]) List<TypeParameter> typeParameters,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
    @Default(<SyntheticImpl>[]) List<SyntheticImpl> syntheticImpls,
  }) = ClassDeclaration;
  const factory Declaration.constructor(
    DeclarationId id, {
    @Default(<TypeParameter>[]) List<TypeParameter> typeParameters,
    @Default(<ValueParameter>[]) List<ValueParameter> valueParameters,
  }) = ConstructorDeclaration;

  const factory Declaration.function(
    DeclarationId id, {
    @required bool isStatic,
    @required bool isTest,
    @required String name,
    @Default(<TypeParameter>[]) List<TypeParameter> typeParameters,
    @Default(<ValueParameter>[]) List<ValueParameter> valueParameters,
    @required CandyType returnType,
  }) = FunctionDeclaration;

  const factory Declaration.property(
    DeclarationId id, {
    @required bool isStatic,
    @required bool isMutable,
    @required String name,
    @required CandyType type,
    Expression initializer,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = PropertyDeclaration;

  const Declaration._();

  FunctionCandyType get functionType {
    assert(this is FunctionDeclaration);
    final function = this as FunctionDeclaration;
    return FunctionCandyType(
      // TODO(JonasWanke): generics
      parameterTypes: function.valueParameters.map((p) => p.type).toList(),
      returnType: function.returnType,
    );
  }
}

@freezed
abstract class SyntheticImpl implements _$SyntheticImpl {
  const factory SyntheticImpl({
    ImplDeclaration implHir,
    List<Tuple2<FunctionDeclaration, List<Expression>>> methods,
  }) = _SyntheticImpl;
  const SyntheticImpl._();
}

@freezed
abstract class TypeParameter implements _$TypeParameter {
  const factory TypeParameter({
    @required String name,
    @required CandyType upperBound,
    CandyType defaultValue,
  }) = _TypeParameter;
  const TypeParameter._();
}

@freezed
abstract class ValueParameter implements _$ValueParameter {
  const factory ValueParameter({
    @required String name,
    @required CandyType type,
    Expression defaultValue,
  }) = _ValueParameter;
  const ValueParameter._();
}
