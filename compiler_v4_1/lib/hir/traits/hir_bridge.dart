import 'package:supernova/supernova.dart';

import '../hir/hir.dart';
import '../hir/ids.dart';
import 'solver.dart';
import 'solver_values.dart';

/// `null` for function and error types, which the solver can't represent.
SolverType? solverTypeOf(HirType type) => switch (type) {
  HirType_Named(:final declaration, :final typeArguments) => () {
    final parameters = <SolverType>[];
    for (final argument in typeArguments) {
      final converted = solverTypeOf(argument);
      if (converted == null) return null;
      parameters.add(converted);
    }
    return SolverValue(
      name: SolverTypeName.declaration(declaration),
      parameters: parameters,
    ).type;
  }(),
  HirType_Parameter(:final id) => SolverVariable.parameter(id).type,
  HirType_Self(:final baseType) => SolverVariable.parameter(
    TypeParameterId.self(baseType.selfOwner),
  ).type,
  HirType_Function() || HirType_Error() || HirType_Variable() => null,
};

/// The goal that [type] implements [trait], or `null` if either isn't
/// representable in the solver.
SolverGoal? solverGoalOf(HirTrait trait, HirType type) {
  final parameters = <SolverType>[];
  for (final argument in trait.typeArguments) {
    final converted = solverTypeOf(argument);
    if (converted == null) return null;
    parameters.add(converted);
  }

  final self = solverTypeOf(type);
  if (self == null) return null;
  return SolverGoal(
    trait: trait.declaration,
    parameters: [...parameters, self],
  );
}

/// The goal a type parameter's upper bound imposes, e.g. `Compare(?T)` for
/// `[T: Compare]`. `null` when the parameter is unbounded.
SolverGoal? solverGoalOfBound(HirTypeParameter parameter) {
  final bound = parameter.upperBound;
  if (bound is! Some<HirTrait>) return null;
  return solverGoalOf(bound.some, parameter.type);
}

/// Replaces the goal's type-parameter variables with what [substitutions] maps
/// them to. Anything the solver can't represent is left as-is.
SolverGoal substituteGoal(
  SolverGoal goal,
  Map<TypeParameterId, HirType> substitutions,
) => SolverGoal(
  trait: goal.trait,
  parameters: goal.parameters.map((it) {
    if (it is! SolverType_Variable) return it;
    final id = it.variable.id;
    if (id is! SolverVariableId_Parameter) return it;
    final replacement = substitutions[id.id];
    return replacement == null ? it : (solverTypeOf(replacement) ?? it);
  }).toList(),
);

List<SolverGoal> solverGoalsOfBounds(List<HirTypeParameter> parameters) =>
    parameters.map(solverGoalOfBound).nonNulls.toList();

Set<SolverVariable> solverVariablesOf(List<HirTypeParameter> parameters) =>
    parameters.map((it) => SolverVariable.parameter(it.id)).toSet();
