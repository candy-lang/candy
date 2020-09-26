import 'package:freezed_annotation/freezed_annotation.dart';

import 'ids.dart';
import 'type.dart';

part 'declarations.freezed.dart';
part 'declarations.g.dart';

@freezed
abstract class Declaration implements _$Declaration {
  const factory Declaration.module({
    ModuleId parent,
    @required String name,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarations,
  }) = ModuleDeclaration;

  const factory Declaration.trait(
    String name, {
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarations,
  }) = TraitDeclaration;

  const factory Declaration.function({
    @required String name,
    @Default(<FunctionParameter>[]) List<FunctionParameter> parameters,
    @required CandyType returnType,
  }) = FunctionDeclaration;

  factory Declaration.fromJson(Map<String, dynamic> json) =>
      _$DeclarationFromJson(json);
  const Declaration._();
}

@freezed
abstract class FunctionParameter implements _$FunctionParameter {
  const factory FunctionParameter({
    @required String name,
    @required CandyType type,
    // TODO(JonasWanke): default value
  }) = _FunctionParameter;
  factory FunctionParameter.fromJson(Map<String, dynamic> json) =>
      _$FunctionParameterFromJson(json);
  const FunctionParameter._();
}
