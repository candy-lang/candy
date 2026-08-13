import 'package:supernova/supernova.dart';

part 'ids.freezed.dart';

/// One source file. [path] is relative to the package root, without `.candy`.
@freezed
sealed class ModuleId with _$ModuleId implements Comparable<ModuleId> {
  const factory ModuleId({
    required String packageName,
    required List<String> path,
  }) = _ModuleId;
  const ModuleId._();

  static const builtin = ModuleId(packageName: r'$builtin', path: []);

  @override
  int compareTo(ModuleId other) {
    final byPackage = packageName.compareTo(other.packageName);
    if (byPackage != 0) return byPackage;
    for (var i = 0; i < path.length && i < other.path.length; i++) {
      final bySegment = path[i].compareTo(other.path[i]);
      if (bySegment != 0) return bySegment;
    }
    return path.length.compareTo(other.path.length);
  }

  @override
  String toString() => [packageName, ...path].join('.');
}

@freezed
sealed class DeclarationId
    with _$DeclarationId
    implements Comparable<DeclarationId> {
  const factory DeclarationId({
    required ModuleId module,
    required String name,
    DeclarationId? parent,
    @Default(0) int disambiguator,
  }) = _DeclarationId;
  const DeclarationId._();

  static const implName = r'$impl';

  DeclarationId child(String name, {int disambiguator = 0}) => DeclarationId(
    module: module,
    name: name,
    parent: this,
    disambiguator: disambiguator,
  );

  @override
  int compareTo(DeclarationId other) {
    final byModule = module.compareTo(other.module);
    if (byModule != 0) return byModule;

    final byParent = switch ((parent, other.parent)) {
      (null, null) => 0,
      (null, _) => -1,
      (_, null) => 1,
      (final a?, final b?) => a.compareTo(b),
    };
    if (byParent != 0) return byParent;

    final byName = name.compareTo(other.name);
    if (byName != 0) return byName;

    return disambiguator.compareTo(other.disambiguator);
  }

  @override
  String toString() {
    final qualified = parent == null ? '$module:$name' : '$parent.$name';
    return disambiguator == 0 ? qualified : '$qualified#$disambiguator';
  }
}

@freezed
sealed class TypeParameterId with _$TypeParameterId {
  const factory TypeParameterId.declared({
    required DeclarationId owner,
    required int index,
    required String name,
  }) = TypeParameterId_Declared;
  const factory TypeParameterId.self(DeclarationId owner) =
      TypeParameterId_Self;

  const TypeParameterId._();

  String get name => switch (this) {
    TypeParameterId_Declared(:final name) => name,
    TypeParameterId_Self() => 'Self',
  };

  @override
  String toString() => name;
}

/// An unknown type during inference. Never survives into the final HIR.
@freezed
sealed class TypeVariableId with _$TypeVariableId {
  const factory TypeVariableId(int value) = _TypeVariableId;
  const TypeVariableId._();

  @override
  String toString() => '?$value';
}

/// An expression or parameter within a single body.
@freezed
sealed class HirId with _$HirId implements Comparable<HirId> {
  const factory HirId(int value) = _HirId;
  const HirId._();

  @override
  int compareTo(HirId other) => value.compareTo(other.value);

  @override
  String toString() => '_$value';
}

class HirIdGenerator {
  var _next = 0;

  HirId generate() => HirId(_next++);
}
