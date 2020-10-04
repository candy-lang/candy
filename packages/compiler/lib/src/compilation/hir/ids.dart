import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';
import 'package:freezed_annotation/freezed_annotation.dart';

import '../ast.dart';
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

  Iterable<DeclarationPathData> get simplePath => path.map((d) => d.data);

  DeclarationId inner(DeclarationPathData innerPath, [int disambiguator = 0]) {
    final disambiguatedInnerPath =
        DisambiguatedDeclarationPathData(innerPath, disambiguator);
    return copyWith(path: path + [disambiguatedInnerPath]);
  }
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
  const factory DeclarationPathData.impl([String name]) =
      ImplDeclarationPathData;
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
  const factory DeclarationLocalId(DeclarationId declarationId, int value) =
      _DeclarationLocalId;
  factory DeclarationLocalId.fromJson(Map<String, dynamic> json) =>
      _$DeclarationLocalIdFromJson(json);
  const DeclarationLocalId._();
}

@freezed
abstract class BodyAstToHirIds implements _$BodyAstToHirIds {
  const factory BodyAstToHirIds([
    @Default(<int, DeclarationLocalId>{}) Map<int, DeclarationLocalId> map,
  ]) = _BodyAstToHirIds;
  factory BodyAstToHirIds.fromJson(Map<String, dynamic> json) =>
      _$BodyAstToHirIdsFromJson(json);
  const BodyAstToHirIds._();

  BodyAstToHirIds withMapping(int astId, DeclarationLocalId hirId) =>
      BodyAstToHirIds({...map, astId: hirId});
}

@freezed
abstract class ModuleId implements _$ModuleId {
  const factory ModuleId(PackageId packageId, List<String> path) = _ModuleId;
  factory ModuleId.fromJson(Map<String, dynamic> json) =>
      _$ModuleIdFromJson(json);
  const ModuleId._();

  static const corePrimitives = ModuleId(PackageId.core, ['primitives']);
  static const coreCollections = ModuleId(PackageId.core, ['collections']);

  static const thisSegment = 'this';
  static const superSegment = 'super';

  bool get hasParent => normalized.path.isNotEmpty;
  bool get hasNoParent => !hasParent;
  ModuleId get parent {
    final parent = parentOrNull;
    assert(parent != null);
    return parent;
  }

  ModuleId get parentOrNull =>
      hasParent ? normalized.copyWith(path: normalized.path.dropLast(1)) : null;

  ModuleId get normalized {
    final result = <String>[];
    for (final rawSegment in path) {
      final segment = rawSegment.trim();

      if (segment == thisSegment) continue;
      if (segment == superSegment) {
        assert(
          result.isNotEmpty,
          'ModuleId containing `super` navigates out of the package.',
        );
        result.removeLast();
        continue;
      }
      result.add(segment);
    }
    return ModuleId(packageId, result);
  }

  ModuleId nested(List<String> innerPath) =>
      copyWith(path: path + innerPath).normalized;
}
