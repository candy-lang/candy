import '../hir/hir.dart';
import '../hir/ids.dart';
import 'projections.dart';
import 'solver.dart';
import 'solver_values.dart';

/// `null` for function and error types, which the solver can't represent.
///
/// A projection is normalized first; one that survives becomes an opaque value
/// standing for the single unknown type it denotes, so a bound over it can be
/// discharged by the same projection appearing among the conditions.
SolverType? solverTypeOf(HirType type, {TypeNormalizer? normalizer}) {
  final normalized = normalizer == null ? type : normalizer.normalize(type);
  return _solverTypeOf(normalized);
}

SolverType? _solverTypeOf(HirType type) => switch (type) {
  HirType_Named(:final declaration, :final typeArguments) => _value(
    SolverTypeName.declaration(declaration),
    typeArguments,
  ),
  HirType_Parameter(:final id) => SolverVariable.parameter(id).type,
  HirType_Self(:final baseType) => _solverTypeOf(baseType),
  HirType_Associated(:final trait, :final baseType, :final declaration) =>
    _value(SolverTypeName.synthetic('\$Assoc\$$declaration'), [
      baseType,
      ...trait.typeArguments,
    ]),
  HirType_Dyn(:final trait) => _value(
    SolverTypeName.synthetic('\$Dyn\$${trait.declaration}'),
    trait.typeArguments,
  ),
  HirType_Function() || HirType_Error() || HirType_Variable() => null,
};

SolverType? _value(SolverTypeName name, List<HirType> arguments) {
  final parameters = <SolverType>[];
  for (final argument in arguments) {
    final converted = _solverTypeOf(argument);
    if (converted == null) return null;
    parameters.add(converted);
  }
  return SolverValue(name: name, parameters: parameters).type;
}

/// The goal that [type] implements [trait], or `null` if either isn't
/// representable in the solver.
SolverGoal? solverGoalOf(
  HirTrait trait,
  HirType type, {
  TypeNormalizer? normalizer,
}) {
  final parameters = <SolverType>[];
  for (final argument in trait.typeArguments) {
    final converted = solverTypeOf(argument, normalizer: normalizer);
    if (converted == null) return null;
    parameters.add(converted);
  }

  final self = solverTypeOf(type, normalizer: normalizer);
  if (self == null) return null;
  return SolverGoal(
    trait: trait.declaration,
    parameters: [...parameters, self],
  );
}

/// Replaces the goal's type-parameter variables with what [substitutions] maps
/// them to. Anything the solver can't represent is left as-is.
SolverGoal substituteGoal(
  SolverGoal goal,
  Map<TypeParameterId, HirType> substitutions, {
  TypeNormalizer? normalizer,
}) => SolverGoal(
  trait: goal.trait,
  parameters: goal.parameters.map((it) {
    if (it is! SolverType_Variable) return it;
    final id = it.variable.id;
    if (id is! SolverVariableId_Parameter) return it;
    final replacement = substitutions[id.id];
    if (replacement == null) return it;
    return solverTypeOf(replacement, normalizer: normalizer) ?? it;
  }).toList(),
);

/// The goals the type parameters' upper bounds impose, e.g. `Compare(?T)` for
/// `[T: Compare]`.
List<SolverGoal> solverGoalsOfBounds(
  List<HirTypeParameter> parameters, {
  TypeNormalizer? normalizer,
}) => [
  for (final parameter in parameters)
    for (final bound in parameter.upperBounds)
      ?solverGoalOf(bound, parameter.type, normalizer: normalizer),
];

Set<SolverVariable> solverVariablesOf(List<HirTypeParameter> parameters) =>
    parameters.map((it) => SolverVariable.parameter(it.id)).toSet();
