import 'package:freezed_annotation/freezed_annotation.dart';

import '../ast/parser.dart';
import '../ids.dart';

part 'ids.freezed.dart';
part 'ids.g.dart';

@freezed
abstract class DeclarationId implements _$DeclarationId {
  const factory DeclarationId(
    ResourceId resourceId,
    List<DisambiguatedDeclarationPathData> path,
  ) = _DeclarationId;
  factory DeclarationId.fromJson(Map<String, dynamic> json) =>
      _$DeclarationIdFromJson(json);
  const DeclarationId._();
}

@freezed
abstract class DisambiguatedDeclarationPathData
    implements _$DisambiguatedDeclarationPathData {
  const factory DisambiguatedDeclarationPathData(
    DeclarationPathData data,
    int disambiguator,
  ) = _DisambiguatedDeclarationPathData;
  factory DisambiguatedDeclarationPathData.fromJson(
          Map<String, dynamic> json) =>
      _$DisambiguatedDeclarationPathDataFromJson(json);
  const DisambiguatedDeclarationPathData._();
}

@freezed
abstract class DeclarationPathData implements _$DeclarationPathData {
  const factory DeclarationPathData.module(String name) =
      ModuleDeclarationPathData;

  const factory DeclarationPathData.trait(String name) =
      TraitDeclarationPathData;
  const factory DeclarationPathData.impl(String name) = ImplDeclarationPathData;
  // ignore: non_constant_identifier_names
  const factory DeclarationPathData.class_(String name) =
      ClassDeclarationPathData;

  const factory DeclarationPathData.function(String name) =
      FunctionDeclarationPathData;
  const factory DeclarationPathData.property(String name) =
      PropertyDeclarationPathData;
  const factory DeclarationPathData.propertyGetter() =
      PropertyGetterDeclarationPathData;
  const factory DeclarationPathData.propertySetter() =
      PropertySetterDeclarationPathData;

  factory DeclarationPathData.fromJson(Map<String, dynamic> json) =>
      _$DeclarationPathDataFromJson(json);
  const DeclarationPathData._();
}

@freezed
abstract class HirId implements _$HirId {
  const factory HirId(DeclarationId declarationId, DeclarationLocalId localId) =
      _HirId;
  factory HirId.fromJson(Map<String, dynamic> json) => _$HirIdFromJson(json);
  const HirId._();
}

@freezed
abstract class DeclarationLocalId implements _$DeclarationLocalId {
  const factory DeclarationLocalId(int value) = _DeclarationLocalId;
  factory DeclarationLocalId.fromJson(Map<String, dynamic> json) =>
      _$DeclarationLocalIdFromJson(json);
  const DeclarationLocalId._();
}
