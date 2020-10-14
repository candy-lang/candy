import 'package:freezed_annotation/freezed_annotation.dart';

import 'expressions.dart';
import 'ids.dart';
import 'type.dart';

part 'declarations.freezed.dart';
part 'declarations.g.dart';

@freezed
abstract class Declaration implements _$Declaration {
  const factory Declaration.module({
    ModuleId parent,
    @required String name,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = ModuleDeclaration;

  const factory Declaration.trait(
    String name, {
    @Default(<TypeParameter>[]) List<TypeParameter> typeParameters,
    @required CandyType upperBound,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = TraitDeclaration;

  // ignore: non_constant_identifier_names
  const factory Declaration.class_({
    @required String name,
    @Default(<TypeParameter>[]) List<TypeParameter> typeParameters,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = ClassDeclaration;

  const factory Declaration.function({
    @required bool isStatic,
    @required String name,
    @Default(<ValueParameter>[]) List<ValueParameter> parameters,
    @required CandyType returnType,
  }) = FunctionDeclaration;

  const factory Declaration.property({
    @required bool isStatic,
    @required bool isMutable,
    @required String name,
    @required CandyType type,
    Expression initializer,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarationIds,
  }) = PropertyDeclaration;

  factory Declaration.fromJson(Map<String, dynamic> json) =>
      _$DeclarationFromJson(json);
  const Declaration._();
}

@freezed
abstract class TypeParameter implements _$TypeParameter {
  const factory TypeParameter({
    @required String name,
    @required CandyType upperBound,
    CandyType defaultValue,
  }) = _TypeParameter;
  factory TypeParameter.fromJson(Map<String, dynamic> json) =>
      _$TypeParameterFromJson(json);
  const TypeParameter._();
}

@freezed
abstract class ValueParameter implements _$ValueParameter {
  const factory ValueParameter({
    @required String name,
    @required CandyType type,
    // TODO(JonasWanke): default value
  }) = _ValueParameter;
  factory ValueParameter.fromJson(Map<String, dynamic> json) =>
      _$ValueParameterFromJson(json);
  const ValueParameter._();
}
