import 'package:compiler_v4_1/hir/hir/builtins.dart';
import 'package:compiler_v4_1/hir/hir/hir.dart';
import 'package:compiler_v4_1/hir/hir/ids.dart';
import 'package:compiler_v4_1/hir/inference/inference.dart';
import 'package:test/test.dart';

const _module = ModuleId(packageName: 'test', path: ['main']);
DeclarationId _declaration(String name) =>
    DeclarationId(module: _module, name: name);

HirType _named(String name, [List<HirType> arguments = const []]) =>
    HirType.named(declaration: _declaration(name), typeArguments: arguments);

HirType _parameter(String name, {String owner = 'owner'}) => HirType.parameter(
  TypeParameterId.declared(owner: _declaration(owner), index: 0, name: name),
);

final _wellKnownTypes = WellKnownTypes(
  intId: _declaration('Int'),
  textId: _declaration('Text'),
  neverId: _declaration('Never'),
);

void main() {
  late TypeVariables variables;
  late Unifier unifier;

  setUp(() {
    variables = TypeVariables();
    unifier = Unifier(variables: variables, wellKnownTypes: _wellKnownTypes);
  });

  group('unify', () {
    test('accepts identical named types', () {
      expect(unifier.unify(_named('Int'), _named('Int')), isTrue);
    });

    test('rejects different named types', () {
      expect(unifier.unify(_named('Int'), _named('Text')), isFalse);
    });

    test('binds a variable and resolves it afterwards', () {
      final variable = variables.fresh();
      expect(unifier.unify(variable, _named('Int')), isTrue);
      expect(variables.resolve(variable), _named('Int'));
    });

    test('resolves through a chain of variables', () {
      final a = variables.fresh();
      final b = variables.fresh();
      expect(unifier.unify(a, b), isTrue);
      expect(unifier.unify(b, _named('Int')), isTrue);
      expect(variables.resolve(a), _named('Int'));
    });

    test('unifies nested type arguments', () {
      final variable = variables.fresh();
      expect(
        unifier.unify(
          _named('List', [variable]),
          _named('List', [_named('Int')]),
        ),
        isTrue,
      );
      expect(variables.resolve(variable), _named('Int'));
    });

    test('rejects one variable needing two different types', () {
      final variable = variables.fresh();
      expect(unifier.unify(variable, _named('Int')), isTrue);
      expect(unifier.unify(variable, _named('Text')), isFalse);
    });

    test('rejects an infinite type', () {
      final variable = variables.fresh();
      expect(unifier.unify(variable, _named('List', [variable])), isFalse);
    });

    test('rejects a type argument arity mismatch', () {
      expect(
        unifier.unify(_named('List', [_named('Int')]), _named('List')),
        isFalse,
      );
    });

    test('compares type parameters by identity, not by name', () {
      expect(
        unifier.unify(_parameter('T'), _parameter('T', owner: 'other')),
        isFalse,
      );
      expect(unifier.unify(_parameter('T'), _parameter('T')), isTrue);
    });

    test('unifies function types', () {
      final variable = variables.fresh();
      expect(
        unifier.unify(
          HirType.function(parameterTypes: [variable], returnType: variable),
          HirType.function(
            parameterTypes: [_named('Int')],
            returnType: _named('Int'),
          ),
        ),
        isTrue,
      );
      expect(variables.resolve(variable), _named('Int'));
    });

    test('treats error types as compatible with anything', () {
      expect(unifier.unify(const HirType.error(), _named('Int')), isTrue);
    });

    test('unifies `Self` with what it stands for', () {
      expect(
        unifier.unify(HirType.self(baseType: _named('Int')), _named('Int')),
        isTrue,
      );
    });
  });

  group('assignable', () {
    test('accepts Never in place of any type', () {
      expect(unifier.assignable(_named('Never'), _named('Int')), isTrue);
      expect(unifier.assignable(_named('Int'), _named('Never')), isFalse);
    });

    test('accepts Never nested inside a type argument', () {
      expect(
        unifier.assignable(
          _named('List', [_named('Never')]),
          _named('List', [_named('Int')]),
        ),
        isTrue,
      );
    });

    test('is invariant in named type arguments otherwise', () {
      expect(
        unifier.assignable(
          _named('List', [_named('Int')]),
          _named('List', [_named('Text')]),
        ),
        isFalse,
      );
    });

    test('is contravariant in function parameters', () {
      // `(Never) Int` is usable where `(Int) Int` is expected.
      expect(
        unifier.assignable(
          HirType.function(
            parameterTypes: [_named('Int')],
            returnType: _named('Int'),
          ),
          HirType.function(
            parameterTypes: [_named('Never')],
            returnType: _named('Int'),
          ),
        ),
        isTrue,
      );
    });

    test('binds a variable on either side', () {
      final variable = variables.fresh();
      expect(unifier.assignable(variable, _named('Int')), isTrue);
      expect(variables.resolve(variable), _named('Int'));
    });
  });

  group('finalize', () {
    test('replaces unbound variables with the error type', () {
      final variable = variables.fresh();
      expect(
        variables.finalize(_named('List', [variable])),
        _named('List', [const HirType.error()]),
      );
    });

    test('keeps bound variables', () {
      final variable = variables.fresh();
      unifier.unify(variable, _named('Int'));
      expect(
        variables.finalize(_named('List', [variable])),
        _named('List', [_named('Int')]),
      );
    });

    test('reports whether a type is still unresolved', () {
      final variable = variables.fresh();
      expect(variables.isUnresolved(_named('List', [variable])), isTrue);
      unifier.unify(variable, _named('Int'));
      expect(variables.isUnresolved(_named('List', [variable])), isFalse);
    });
  });
}
