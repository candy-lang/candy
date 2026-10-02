import 'dart:collection';

import 'package:supernova/supernova.dart';

import '../hir/ids.dart';

part 'solver_values.freezed.dart';

/// The solver fabricates types while canonicalizing goals, hence
/// [SolverTypeName.synthetic].
@freezed
sealed class const SolverTypeName._() with _$SolverTypeName {
  const factory declaration(DeclarationId id) = SolverTypeName_Declaration;
  const factory synthetic(String name) = SolverTypeName_Synthetic;

  @override
  String toString() => switch (this) {
    SolverTypeName_Declaration(:final id) => id.name,
    SolverTypeName_Synthetic(:final name) => name,
  };
}

/// [SolverVariableId.canonical] is used when goals get canonicalized so that
/// structurally equal goals hash equally.
@freezed
sealed class const SolverVariableId._()
    with _$SolverVariableId
    implements Comparable<SolverVariableId> {
  const factory parameter(TypeParameterId id) = SolverVariableId_Parameter;
  const factory canonical(int index) = SolverVariableId_Canonical;

  /// Makes unification substitute the same variable whichever order the two
  /// are given in.
  @override
  int compareTo(SolverVariableId other) => switch ((this, other)) {
    (SolverVariableId_Canonical(:final index), SolverVariableId_Canonical()) =>
      index.compareTo((other as SolverVariableId_Canonical).index),
    (SolverVariableId_Canonical(), SolverVariableId_Parameter()) => -1,
    (SolverVariableId_Parameter(), SolverVariableId_Canonical()) => 1,
    (SolverVariableId_Parameter(:final id), SolverVariableId_Parameter()) =>
      () {
        final otherId = (other as SolverVariableId_Parameter).id;
        final byName = id.name.compareTo(otherId.name);
        if (byName != 0) return byName;
        return id.owner.compareTo(otherId.owner);
      }(),
  };

  @override
  String toString() => switch (this) {
    SolverVariableId_Parameter(:final id) => id.name,
    SolverVariableId_Canonical(:final index) => 'canonical\$$index',
  };
}

/// Either a [SolverVariable] or a [SolverValue].
@freezed
sealed class const SolverType._() with _$SolverType {
  const factory variable(SolverVariable variable) = SolverType_Variable;
  const factory value(SolverValue value) = SolverType_Value;

  bool containsVariable(SolverVariable variable) => switch (this) {
    SolverType_Variable(variable: final it) => it == variable,
    SolverType_Value(:final value) => value.parameters.any(
      (it) => it.containsVariable(variable),
    ),
  };

  SolverType substitute(SolverVariable variable, SolverType substitution) =>
      substituteAll({variable: substitution});

  SolverType substituteAll(Map<SolverVariable, SolverType> substitutions) =>
      switch (this) {
        SolverType_Variable(:final variable) => substitutions[variable] ?? this,
        SolverType_Value(:final value) => SolverType.value(
          value.substituteAll(substitutions),
        ),
      };

  SolverType canonicalize() => canonicalizeInternal({});

  SolverType canonicalizeInternal(
    Map<SolverVariable, SolverVariable> mapping,
  ) => switch (this) {
    SolverType_Variable(:final variable) => SolverType.variable(
      mapping.putIfAbsent(
        variable,
        () => SolverVariable(SolverVariableId.canonical(mapping.length)),
      ),
    ),
    SolverType_Value(:final value) => SolverType.value(
      value.canonicalizeInternal(mapping),
    ),
  };

  /// Returns the substitutions that make both types equal, or `null` if none
  /// exist. Unifying `?A` with `Na<?A>` fails: repeated substitution would
  /// build an infinitely large type.
  Map<SolverVariable, SolverType>? unify(SolverType other) {
    switch ((this, other)) {
      case (
        SolverType_Variable(variable: final a),
        SolverType_Variable(variable: final b),
      ):
        if (a == b) return {};
        return a.id.compareTo(b.id) < 0 ? {a: other} : {b: this};

      case (SolverType_Value(), SolverType_Variable()):
        return other.unify(this);

      case (SolverType_Variable(variable: final a), SolverType_Value()):
        if (other.containsVariable(a)) return null;
        return {a: other};

      case (SolverType_Value(value: final a), SolverType_Value(value: final b)):
        if (a.name != b.name || a.parameters.length != b.parameters.length) {
          return null;
        }

        final queue = Queue<(SolverVariable, SolverType)>();
        for (final (index, parameter) in a.parameters.indexed) {
          final result = parameter.unify(b.parameters[index]);
          if (result == null) return null;
          queue.addAll(result.entries.map((it) => (it.key, it.value)));
        }

        final solutionSpace = <SolverVariable, SolverType>{};
        while (queue.isNotEmpty) {
          final (variable, type) = queue.removeFirst();
          final existing = solutionSpace[variable];
          if (existing != null) {
            final result = existing.unify(type);
            if (result == null) return null;
            queue.addAll(result.entries.map((it) => (it.key, it.value)));
            continue;
          }

          solutionSpace[variable] = type;
          solutionSpace.updateAll((_, it) => it.substitute(variable, type));
          final substituted = queue
              .map((it) => (it.$1, it.$2.substitute(variable, type)))
              .toList();
          queue
            ..clear()
            ..addAll(substituted);
          if (queue.any((it) => it.$2.containsVariable(it.$1))) return null;
        }
        return solutionSpace;
    }
  }

  @override
  String toString() => switch (this) {
    SolverType_Variable(:final variable) => variable.toString(),
    SolverType_Value(:final value) => value.toString(),
  };
}

/// Corresponds to a type parameter in Candy.
@freezed
sealed class const SolverVariable._() with _$SolverVariable {
  const factory(SolverVariableId id) = _SolverVariable;

  factory parameter(TypeParameterId id) =>
      SolverVariable(SolverVariableId.parameter(id));

  SolverType get type => SolverType.variable(this);

  @override
  String toString() => '?$id';
}

/// A concrete type, which may contain [SolverVariable]s in its [parameters].
@freezed
sealed class const SolverValue._() with _$SolverValue {
  const factory({
    required SolverTypeName name,
    required List<SolverType> parameters,
  }) = _SolverValue;

  /// Lets whole parameter lists be unified and canonicalized as one value.
  factory placeholder(List<SolverType> types) => SolverValue(
    name: const SolverTypeName.synthetic(r'$Placeholder'),
    parameters: types,
  );

  SolverType get type => SolverType.value(this);

  SolverValue substituteAll(Map<SolverVariable, SolverType> substitutions) =>
      SolverValue(
        name: name,
        parameters: parameters
            .map((it) => it.substituteAll(substitutions))
            .toList(),
      );

  SolverValue canonicalize() => canonicalizeInternal({});

  SolverValue canonicalizeInternal(
    Map<SolverVariable, SolverVariable> mapping,
  ) => SolverValue(
    name: name,
    parameters: parameters
        .map((it) => it.canonicalizeInternal(mapping))
        .toList(),
  );

  @override
  String toString() =>
      parameters.isEmpty ? '$name' : '$name<${parameters.join(', ')}>';
}
