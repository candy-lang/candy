import 'package:freezed_annotation/freezed_annotation.dart';

import 'ids.dart';

part 'declarations.freezed.dart';
part 'declarations.g.dart';

@freezed
abstract class Declaration implements _$Declaration {
  const factory Declaration.module({
    DeclarationId parent,
    @required String name,
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarations,
  }) = ModuleDeclaration;

  const factory Declaration.trait(
    String name, {
    @Default(<DeclarationId>[]) List<DeclarationId> innerDeclarations,
  }) = TraitDeclaration;

  factory Declaration.fromJson(Map<String, dynamic> json) =>
      _$DeclarationFromJson(json);
  const Declaration._();
}
