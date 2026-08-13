import 'dart:collection';

import 'package:supernova/supernova.dart';

import '../hir/ids.dart';
import 'solver_values.dart';

part 'solver.freezed.dart';

/// The fact that a type implements a trait, e.g. `Compare(Ordering)`.
@freezed
sealed class SolverGoal with _$SolverGoal {
  const factory SolverGoal({
    required DeclarationId trait,
    required List<SolverType> parameters,
  }) = _SolverGoal;
  const SolverGoal._();

  Map<SolverVariable, SolverType>? unify(SolverGoal other) {
    if (trait != other.trait) return null;
    return SolverValue.placeholder(
      parameters,
    ).type.unify(SolverValue.placeholder(other.parameters).type);
  }

  SolverGoal substituteAll(Map<SolverVariable, SolverType> substitutions) =>
      SolverGoal(
        trait: trait,
        parameters: parameters
            .map((it) => it.substituteAll(substitutions))
            .toList(),
      );

  SolverGoal canonicalize() => canonicalizeInternal({});

  SolverGoal canonicalizeInternal(
    Map<SolverVariable, SolverVariable> mapping,
  ) => SolverGoal(
    trait: trait,
    parameters: SolverValue.placeholder(
      parameters,
    ).canonicalizeInternal(mapping).parameters,
  );

  @override
  String toString() => '${trait.name}(${parameters.join(', ')})';
}

/// `impl[T: Compare] T: Equal` becomes `Equal(?T) <- Compare(?T)`.
@freezed
sealed class SolverRule with _$SolverRule {
  const factory SolverRule({
    required SolverGoal goal,
    required List<SolverGoal> subgoals,
  }) = _SolverRule;
  const SolverRule._();

  @override
  String toString() =>
      '$goal <- ${subgoals.isEmpty ? '<nothing>' : subgoals.join(', ')}';
}

class SolverEnvironment {
  SolverEnvironment([List<SolverRule>? rules]) : rules = [...?rules];

  final List<SolverRule> rules;

  void addRule(SolverRule rule) => rules.add(rule);

  /// Checks whether [goal] holds, assuming [conditions] hold.
  ///
  /// Free variables are existential: `Equal(?T)` with no conditions asks
  /// whether *some* type implements `Equal` and succeeds as soon as one does.
  /// Pass every in-scope type parameter in [opaqueVariables] to get the
  /// universal reading instead — including unbounded ones, which contribute no
  /// condition of their own and which the Rust compiler therefore misses.
  SolverSolution solve(
    SolverGoal goal,
    List<SolverGoal> conditions, {
    Set<SolverVariable> opaqueVariables = const {},
  }) {
    // Turn each condition variable into a virtual type that implements
    // exactly the conditions' traits, so conditions can be treated as facts.
    // Canonicalizing is only how the virtual types get deterministic names;
    // the substitution stays keyed by the original variables. The Rust
    // compiler keys this map by the original variables but applies it to the
    // canonicalized conditions, so nothing substitutes and each condition
    // stays a free variable that unifies with anything — which reports any
    // goal over a bounded type parameter as ambiguous.
    final canonicalMapping = <SolverVariable, SolverVariable>{};
    for (final variable in opaqueVariables) {
      variable.type.canonicalizeInternal(canonicalMapping);
    }
    for (final condition in conditions) {
      condition.canonicalizeInternal(canonicalMapping);
    }

    final toVirtual = {
      for (final entry in canonicalMapping.entries)
        entry.key: SolverValue(
          name: SolverTypeName.synthetic('\$Virtual\$${entry.value.id}'),
          parameters: const [],
        ).type,
    };
    final virtualRules = conditions.map(
      (it) => SolverRule(goal: it.substituteAll(toVirtual), subgoals: const []),
    );

    return _Solver([
      ...rules,
      ...virtualRules,
    ]).solve(goal.substituteAll(toVirtual));
  }

  @override
  String toString() =>
      ['SolverEnvironment(', ...rules.map((it) => '  $it'), ')'].join('\n');
}

@freezed
sealed class SolverSolution with _$SolverSolution {
  /// There is exactly one type that fulfills the goal.
  const factory SolverSolution.unique({
    required SolverGoal refinedGoal,
    required SolverRule usedRule,
  }) = SolverSolution_Unique;

  /// There may be multiple types that fulfill the goal.
  const factory SolverSolution.ambiguous() = SolverSolution_Ambiguous;

  /// It's impossible to fulfill the goal.
  const factory SolverSolution.impossible() = SolverSolution_Impossible;

  const SolverSolution._();

  bool get isUnique => this is SolverSolution_Unique;

  @override
  String toString() => switch (this) {
    SolverSolution_Unique(:final refinedGoal, :final usedRule) =>
      'Unique(refined goal: $refinedGoal, used rule: $usedRule)',
    SolverSolution_Ambiguous() => 'Ambiguous',
    SolverSolution_Impossible() => 'Impossible',
  };
}

class _Solver {
  _Solver(this.rules);

  final List<SolverRule> rules;

  final _solved = <SolverGoal, SolverSolution>{};
  final _beingSolved = <SolverGoal>{};

  SolverSolution solve(SolverGoal goal) {
    final canonical = goal.canonicalize();

    final cached = _solved[canonical];
    if (cached != null) return cached;

    if (!_beingSolved.add(canonical)) {
      // Cycle: break it by treating the goal as ambiguous.
      _solved[canonical] = const SolverSolution.ambiguous();
      return const SolverSolution.ambiguous();
    }

    final solution = _solveTree(canonical);
    _beingSolved.remove(canonical);
    _solved[canonical] = solution;
    return solution;
  }

  SolverSolution _solveTree(SolverGoal goal) {
    final strands = <_SolverStrand>[];
    for (final rule in rules) {
      final result = rule.goal.unify(goal);
      if (result == null) continue;

      strands.add(
        _SolverStrand(
          usedRule: rule,
          goal: rule.goal.substituteAll(result),
          solutionSpace: {...result},
          subgoals: Queue.of(
            rule.subgoals.map((it) => it.substituteAll(result)),
          ),
        ),
      );
    }

    final solved = <(_SolverStrand, SolverSolution)>[];
    for (final strand in strands) {
      final solution = strand.solve(this);
      if (solution is SolverSolution_Impossible) continue;
      solved.add((strand, solution));
    }

    if (solved.length > 1) return const SolverSolution.ambiguous();
    if (solved.isEmpty) return const SolverSolution.impossible();

    final (strand, solution) = solved.single;
    if (solution is! SolverSolution_Unique) {
      return const SolverSolution.ambiguous();
    }

    final substitutions = strand.goal.unify(solution.refinedGoal);
    if (substitutions == null) return const SolverSolution.ambiguous();
    return SolverSolution.unique(
      refinedGoal: goal.substituteAll(substitutions),
      usedRule: solution.usedRule,
    );
  }
}

class _SolverStrand {
  _SolverStrand({
    required this.usedRule,
    required this.goal,
    required this.solutionSpace,
    required this.subgoals,
  });

  final SolverRule usedRule;
  final SolverGoal goal;
  final Map<SolverVariable, SolverType> solutionSpace;
  final Queue<SolverGoal> subgoals;

  SolverSolution solve(_Solver solver) {
    while (subgoals.isNotEmpty) {
      final subgoal = subgoals.removeFirst().canonicalize();
      final solution = solver.solve(subgoal);
      switch (solution) {
        case SolverSolution_Impossible():
          return const SolverSolution.impossible();
        case SolverSolution_Ambiguous():
          continue;
        case SolverSolution_Unique(:final refinedGoal):
          final substitutions = subgoal.unify(refinedGoal);
          if (substitutions == null) return const SolverSolution.impossible();
          for (final MapEntry(key: variable, value: type)
              in substitutions.entries) {
            final existing = solutionSpace[variable];
            if (existing == null) {
              solutionSpace[variable] = type;
              continue;
            }

            final refined = existing.unify(type);
            if (refined == null) return const SolverSolution.impossible();
            solutionSpace.updateAll((_, it) => it.substituteAll(refined));
          }
      }
    }

    return SolverSolution.unique(
      refinedGoal: goal.substituteAll(solutionSpace),
      usedRule: usedRule,
    );
  }
}
