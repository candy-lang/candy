import 'package:test/test.dart';

import '../hir/lowering/harness.dart';

void main() {
  test('resolves a local binding', () {
    final harness = _lower('''
      fun main() Int {
        let value = 1
        value
      }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('value', occurrence: 2), [
      harness.location('value'),
    ]);
  });

  test('resolves a parameter', () {
    final harness = _lower('''
      fun identity(input: Int) Int { input }
      fun main() Int { identity(1) }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('input', occurrence: 2), [
      harness.location('input'),
    ]);
  });

  test('resolves a lambda parameter', () {
    final harness = _lower('''
      fun main() Int {
        let double = (factor: Int) { factor }
        double(2)
      }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('factor', occurrence: 2), [
      harness.location('factor'),
    ]);
  });

  test('resolves a switch binding', () {
    final harness = _lower('''
      fun main() Int {
        switch Maybe.some[Int](1) {
          none => 0,
          some(inner) => inner,
        }
      }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('inner', occurrence: 2), [
      harness.location('inner'),
    ]);
  });

  test('resolves a called function', () {
    final harness = _lower('''
      fun helper() Int { 1 }
      fun main() Int { helper() }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('helper', occurrence: 2), [
      harness.location('helper'),
    ]);
  });

  test('resolves a type name', () {
    final harness = _lower('''
      struct Point { x: Int, y: Int }
      fun main() Int { Point(1, 2).x }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('Point', occurrence: 2), [
      harness.location('Point'),
    ]);
  });

  test('resolves a struct field', () {
    final harness = _lower('''
      struct Point { first: Int, second: Int }
      fun main() Int { Point(1, 2).second }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('second', occurrence: 2), [
      harness.location('second'),
    ]);
  });

  test('resolves a type in a signature', () {
    final harness = _lower('''
      struct Point { x: Int, y: Int }
      fun origin() Point { Point(0, 0) }
      fun main() Int { origin().x }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('Point', occurrence: 2), [
      harness.location('Point'),
    ]);
  });

  test('resolves an enum variant', () {
    final harness = _lower('''
      enum Color { red, green }
      fun main() Int { switch Color.red() { red => 0, green => 1 } }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('red', occurrence: 2), [
      harness.location('red'),
    ]);
    expect(harness.definitionsAt('red', occurrence: 3), [
      harness.location('red'),
    ]);
  });

  test('resolves a type parameter to its declaration', () {
    final harness = _lower('''
      fun identity[Element](value: Element) Element { value }
      fun main() Int { identity[Int](1) }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('Element)'), [harness.location('Element')]);
  });

  test('resolves a trait name and a method call', () {
    final harness = _lower('''
      trait Greet { fun greet(self: Self) Int }
      struct Person {}
      impl Person: Greet { fun greet(self: Self) Int { 1 } }
      fun main() Int { Person().greet() }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('Greet', occurrence: 2), [
      harness.location('Greet'),
    ]);
    expect(harness.definitionsAt('greet', occurrence: 3), [
      harness.location('greet', occurrence: 2),
    ]);
  });

  test('resolves a top-level assignment', () {
    final harness = _lower('''
      let answer: Int = 42
      fun main() Int { answer }
    ''');
    harness.expectNoErrors();
    expect(harness.definitionsAt('answer', occurrence: 2), [
      harness.location('answer'),
    ]);
  });

  test('resolves a use line and a call into the imported module', () {
    final harness = LoweringHarness.fullModules({
      'helpers':
          'package struct Value {}\n'
          'package fun helper() Value { Value() }',
      'main':
          'use package.helpers\n'
          'struct Int = builtin\n'
          'fun f() Value { helper() }\n'
          'fun main() Int { 0 }',
    });
    harness.expectNoErrors();
    expect(harness.definitionsAt('package.helpers', path: 'main'), ['@1']);
    expect(harness.definitionsAt('helper', occurrence: 2, path: 'main'), [
      harness.location('helper', path: 'helpers'),
    ]);
  });

  test('offers every candidate when a call cannot be resolved', () {
    final harness = _lower('''
      fun ambiguous(value: Int) Int { 1 }
      fun ambiguous(value: Text) Int { 2 }
      fun main() Int { ambiguous(Nothing()) }
    ''');
    expect(harness.errorMessages, hasLength(1));
    expect(harness.definitionsAt('ambiguous', occurrence: 3), [
      harness.location('ambiguous'),
      harness.location('ambiguous', occurrence: 2),
    ]);
  });

  test('has nothing to say about a name that does not resolve', () {
    final harness = _lower('fun main() Int { missing }');
    expect(harness.errorMessages, hasLength(1));
    expect(harness.definitionsAt('missing'), isEmpty);
  });

  group('find references', () {
    test(
      'lists every use of a function, from a use and from the declaration',
      () {
        final harness = _lower('''
        fun helper() Int { 1 }
        fun a() Int { helper() }
        fun b() Int { helper() }
        fun main() Int { 0 }
      ''');
        harness.expectNoErrors();

        final uses = [
          harness.location('helper', occurrence: 2),
          harness.location('helper', occurrence: 3),
        ];
        expect(harness.referencesAt('helper', occurrence: 2), uses);
        expect(harness.referencesAt('helper'), uses);
      },
    );

    test('includes the declaration when asked', () {
      final harness = _lower('''
        fun helper() Int { 1 }
        fun main() Int { helper() }
      ''');
      harness.expectNoErrors();
      expect(harness.referencesAt('helper', includeDeclaration: true), [
        harness.location('helper'),
        harness.location('helper', occurrence: 2),
      ]);
    });

    test('keeps two locals of the same name apart', () {
      final harness = _lower('''
        fun a() Int {
          let value = 1
          value
        }
        fun b() Int {
          let value = 2
          value
        }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
      expect(harness.referencesAt('value'), [
        harness.location('value', occurrence: 2),
      ]);
      expect(harness.referencesAt('value', occurrence: 3), [
        harness.location('value', occurrence: 4),
      ]);
    });

    test('lists uses of a type in signatures and constructions', () {
      final harness = _lower('''
        struct Point { first: Int, second: Int }
        fun origin() Point { Point(0, 0) }
        fun main() Int { origin().first }
      ''');
      harness.expectNoErrors();
      expect(harness.referencesAt('Point'), [
        harness.location('Point', occurrence: 2),
        harness.location('Point', occurrence: 3),
      ]);
    });

    test('lists uses of a struct field', () {
      final harness = _lower('''
        struct Point { first: Int, second: Int }
        fun main() Int { Point(1, 2).first }
      ''');
      harness.expectNoErrors();
      expect(harness.referencesAt('first'), [
        harness.location('first', occurrence: 2),
      ]);
    });

    test('lists uses across modules', () {
      final harness = LoweringHarness.fullModules({
        'helpers':
            'package struct Value {}\n'
            'package fun helper() Value { Value() }',
        'main':
            'use package.helpers\n'
            'struct Int = builtin\n'
            'fun f() Value { helper() }\n'
            'fun main() Int { 0 }',
      });
      harness.expectNoErrors();
      expect(harness.referencesAt('helper', path: 'helpers'), [
        harness.location('helper', occurrence: 2, path: 'main'),
      ]);
    });

    test(
      'lists a call that could not be narrowed down under each candidate',
      () {
        final harness = _lower('''
        fun ambiguous(value: Int) Int { 1 }
        fun ambiguous(value: Text) Int { 2 }
        fun main() Int { ambiguous(Nothing()) }
      ''');
        expect(harness.errorMessages, hasLength(1));

        final call = [harness.location('ambiguous', occurrence: 3)];
        expect(harness.referencesAt('ambiguous'), call);
        expect(harness.referencesAt('ambiguous', occurrence: 2), call);
      },
    );

    test('finds a declaration nothing uses', () {
      final harness = _lower('''
        fun unused() Int { 1 }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
      expect(harness.referencesAt('unused'), isEmpty);
      expect(harness.referencesAt('unused', includeDeclaration: true), [
        harness.location('unused'),
      ]);
    });

    test('finds unused declarations of every kind', () {
      final harness = _lower('''
        struct Unused[Element] { field: Int }
        enum Unusable { variant }
        trait Untouched { fun method(self: Self) Int }
        let constant: Int = 1
        fun withParameter(argument: Int) Int { 0 }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();

      for (final name in [
        'Unused',
        'Element',
        'field',
        'Unusable',
        'variant',
        'Untouched',
        'method',
        'constant',
        'withParameter',
        'argument',
      ]) {
        expect(
          harness.referencesAt(name, includeDeclaration: true),
          [harness.location(name)],
          reason: 'Expected to find the declaration of `$name`.',
        );
      }
    });

    test('finds unused bindings inside a body', () {
      final harness = _lower('''
        fun main() Int {
          let binding = 1
          let lambda = (parameter: Int) { 0 }
          switch Maybe.some[Int](1) {
            none => 0,
            some(bound) => 0,
          }
        }
      ''');
      harness.expectNoErrors();

      for (final name in ['binding', 'parameter', 'bound']) {
        expect(
          harness.referencesAt(name, includeDeclaration: true),
          [harness.location(name)],
          reason: 'Expected to find the declaration of `$name`.',
        );
      }
    });
  });
}

LoweringHarness _lower(String source) =>
    LoweringHarness.full('$preludeSource$source');
