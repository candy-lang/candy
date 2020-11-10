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
    ResourceId resourceId, [
    @Default(<DisambiguatedDeclarationPathData>[])
        List<DisambiguatedDeclarationPathData> path,
  ]) = _DeclarationId;
  factory DeclarationId.fromJson(Map<String, dynamic> json) =>
      _$DeclarationIdFromJson(json);
  const DeclarationId._();

  Iterable<DeclarationPathData> get simplePath => path.map((d) => d.data);

  DeclarationId inner(DeclarationPathData innerPath, [int disambiguator = 0]) {
    final disambiguatedInnerPath =
        DisambiguatedDeclarationPathData(innerPath, disambiguator);
    return copyWith(path: path + [disambiguatedInnerPath]);
  }

  bool get hasParent => path.isNotEmpty;
  DeclarationId get parent {
    assert(hasParent);
    return copyWith(path: path.dropLast(1));
  }

  @override
  String toString() => '$resourceId:${path.join('.')}';
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

  @override
  String toString() =>
      disambiguator == 0 ? data.toString() : '$data#$disambiguator';
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
  const factory DeclarationPathData.constructor() =
      ConstructorDeclarationPathData;

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

  String get nameOrNull => when(
        module: (name) => name,
        trait: (name) => name,
        impl: (name) => name,
        class_: (name) => name,
        constructor: () => null,
        function: (name) => name,
        property: (name) => name,
        propertyGetter: () => null,
        propertySetter: () => null,
      );

  @override
  String toString() {
    return when(
      module: (name) => 'mod($name)',
      trait: (name) => 'trait($name)',
      impl: (name) => 'impl($name)',
      class_: (name) => 'class($name)',
      constructor: () => 'constructor',
      function: (name) => 'fun($name)',
      property: (name) => 'prop($name)',
      propertyGetter: () => 'get',
      propertySetter: () => 'set',
    );
  }
}

@freezed
abstract class DeclarationLocalId implements _$DeclarationLocalId {
  const factory DeclarationLocalId(DeclarationId declarationId, int value) =
      _DeclarationLocalId;
  factory DeclarationLocalId.fromJson(Map<String, dynamic> json) =>
      _$DeclarationLocalIdFromJson(json);
  const DeclarationLocalId._();

  @override
  String toString() => '$declarationId+$value';
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
  const factory ModuleId(
    PackageId packageId, [
    @Default(<String>[]) List<String> path,
  ]) = _ModuleId;
  factory ModuleId.fromJson(Map<String, dynamic> json) =>
      _$ModuleIdFromJson(json);
  const ModuleId._();

  static const core = ModuleId(PackageId.core);
  static const coreAssert = ModuleId(PackageId.core, ['assert']);
  static const coreCollections = ModuleId(PackageId.core, ['collections']);
  static const coreOperators = ModuleId(PackageId.core, ['operators']);
  static const coreOperatorsArithmetic =
      ModuleId(PackageId.core, ['operators', 'arithmetic']);
  static const coreOperatorsComparison =
      ModuleId(PackageId.core, ['operators', 'comparison']);
  static const coreOperatorsEquality =
      ModuleId(PackageId.core, ['operators', 'equality']);
  static const coreOperatorsLogical =
      ModuleId(PackageId.core, ['operators', 'logical']);
  static const corePrimitives = ModuleId(PackageId.core, ['primitives']);
  static const coreReflection = ModuleId(PackageId.core, ['reflection']);
  static const coreIo = ModuleId(PackageId.core, ['io']);
  static const corePrint = ModuleId(PackageId.core, ['io', 'print']);

  bool get hasParent => path.isNotEmpty;
  bool get hasNoParent => !hasParent;
  ModuleId get parent {
    final parent = parentOrNull;
    assert(parent != null);
    return parent;
  }

  ModuleId get parentOrNull =>
      hasParent ? copyWith(path: path.dropLast(1)) : null;

  ModuleId nested(List<String> innerPath) => copyWith(path: path + innerPath);

  @override
  String toString() => '$packageId:${path.join('.')}';
}
