import 'package:compiler_v4_1/hir/hir/ids.dart';
import 'package:compiler_v4_1/hir/traits/solver.dart';
import 'package:compiler_v4_1/hir/traits/solver_values.dart';
import 'package:test/test.dart';

const _module = ModuleId(packageName: 'test', path: ['main']);

DeclarationId _declaration(String name) =>
    DeclarationId(module: _module, name: name);

/// A concrete type like `Ordering` or `List[Int]`.
SolverType _type(String name, [List<SolverType> arguments = const []]) =>
    SolverValue(
      name: SolverTypeName.declaration(_declaration(name)),
      parameters: arguments,
    ).type;

/// A type variable standing for the type parameter [name] of [owner].
SolverType _variable(String name, {String owner = 'owner'}) =>
    SolverVariable.parameter(
      TypeParameterId.declared(
        owner: _declaration(owner),
        index: 0,
        name: name,
      ),
    ).type;

SolverGoal _goal(String trait, List<SolverType> parameters) =>
    SolverGoal(trait: _declaration(trait), parameters: parameters);

void main() {
  group('SolverType.unify', () {
    test('unifies identical concrete types without substitutions', () {
      expect(_type('Int').unify(_type('Int')), isEmpty);
    });

    test('fails on different concrete types', () {
      expect(_type('Int').unify(_type('Text')), isNull);
    });

    test('binds a variable to a concrete type, in both directions', () {
      final a = _variable('A');
      expect(a.unify(_type('Int')), {
        (a as SolverType_Variable).variable: _type('Int'),
      });
      expect(_type('Int').unify(a), {a.variable: _type('Int')});
    });

    test('unifies nested type arguments', () {
      final a = _variable('A');
      final result = _type('List', [a]).unify(
        _type('List', [
          _type('Set', [_type('Int')]),
        ]),
      );
      expect(result, {
        (a as SolverType_Variable).variable: _type('Set', [_type('Int')]),
      });
    });

    test('fails when one variable would need two different types', () {
      final a = _variable('A');
      expect(
        _type('Map', [a, a]).unify(_type('Map', [_type('Int'), _type('Text')])),
        isNull,
      );
    });

    test('succeeds when one variable is used twice consistently', () {
      final a = _variable('A');
      expect(
        _type('Map', [a, a]).unify(_type('Map', [_type('Int'), _type('Int')])),
        {(a as SolverType_Variable).variable: _type('Int')},
      );
    });

    test('rejects infinitely growing types', () {
      final a = _variable('A');
      expect(a.unify(_type('Na', [a])), isNull);
    });

    test('fails on differing arity', () {
      expect(_type('List', [_type('Int')]).unify(_type('List')), isNull);
    });
  });

  group('SolverEnvironment.solve', () {
    // Mirrors example.candy:
    //   impl Ordering: Compare { … }
    //   impl[T: Compare] T: Equal { … }
    late SolverEnvironment environment;

    setUp(() {
      environment = SolverEnvironment([
        // Compare(Ordering) <- ∅
        SolverRule(
          goal: _goal('Compare', [_type('Ordering')]),
          subgoals: const [],
        ),
        // Equal(?T) <- Compare(?T)
        SolverRule(
          goal: _goal('Equal', [_variable('T')]),
          subgoals: [
            _goal('Compare', [_variable('T')]),
          ],
        ),
      ]);
    });

    test('proves a goal that a direct impl provides', () {
      expect(
        environment.solve(_goal('Compare', [_type('Ordering')]), const []),
        isA<SolverSolution_Unique>(),
      );
    });

    test('proves a goal reached through a blanket impl', () {
      expect(
        environment.solve(_goal('Equal', [_type('Ordering')]), const []),
        isA<SolverSolution_Unique>(),
      );
    });

    test('rejects a goal whose subgoal cannot be proven', () {
      // `Int` has no `Compare` impl, so the blanket `Equal` impl doesn't apply.
      expect(
        environment.solve(_goal('Equal', [_type('Int')]), const []),
        const SolverSolution.impossible(),
      );
    });

    test('rejects a goal with no matching rule at all', () {
      expect(
        environment.solve(_goal('ToText', [_type('Ordering')]), const []),
        const SolverSolution.impossible(),
      );
    });

    test('proves a goal about a variable given a matching condition', () {
      // Inside `fun f[T: Compare](…)`, `Equal(?T)` holds because `Compare(?T)`
      // is assumed.
      expect(
        environment.solve(_goal('Equal', [_variable('T')]), [
          _goal('Compare', [_variable('T')]),
        ]),
        isA<SolverSolution_Unique>(),
      );
    });

    test('treats a free variable existentially', () {
      // Asks "does *some* type implement Equal?", which `Ordering` does.
      expect(
        environment.solve(_goal('Equal', [_variable('T')]), const []),
        isA<SolverSolution_Unique>(),
      );
    });

    test('rejects a goal about a variable with an unrelated condition', () {
      // Knowing only `ToText(?T)` says nothing about `Equal(?T)`.
      expect(
        environment.solve(_goal('Equal', [_variable('T')]), [
          _goal('ToText', [_variable('T')]),
        ]),
        const SolverSolution.impossible(),
      );
    });

    test('rejects a goal about an opaque, unbounded variable', () {
      // The case of `fun f[T](x: T) { … x.equals(y) … }`: `T` has no bound, so
      // it contributes no condition, but it must still not satisfy `Equal`.
      final variable = _variable('T') as SolverType_Variable;
      expect(
        environment.solve(
          _goal('Equal', [variable]),
          const [],
          opaqueVariables: {variable.variable},
        ),
        const SolverSolution.impossible(),
      );
    });

    test('reports ambiguity when two rules prove the same goal', () {
      environment.addRule(
        SolverRule(
          goal: _goal('Equal', [_type('Ordering')]),
          subgoals: const [],
        ),
      );
      expect(
        environment.solve(_goal('Equal', [_type('Ordering')]), const []),
        isA<SolverSolution_Ambiguous>().having(
          (it) => it.cause,
          'cause',
          isA<SolverAmbiguity_MultipleImpls>().having(
            (it) => it.candidates,
            'candidates',
            hasLength(2),
          ),
        ),
      );
    });
  });
}
