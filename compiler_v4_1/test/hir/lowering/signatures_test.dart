import 'package:compiler_v4_1/hir/hir/hir.dart';
import 'package:test/test.dart';

import 'harness.dart';

void main() {
  group('types', () {
    test('lowers parameter and return types', () {
      final harness = _lower('fun f(a: Int, b: Text) Int { a }');
      harness.expectNoErrors();
      expect(harness.signatureOf('f'), 'f(a: Int, b: Text) Int');
    });

    test('defaults a missing return type to Nothing', () {
      final harness = _lower('fun f(a: Int) { a }');
      harness.expectNoErrors();
      expect(harness.signatureOf('f'), 'f(a: Int) Nothing');
    });

    test('lowers generic types and function types', () {
      final harness = _lower(
        'fun f(a: List[Int], b: (Int, Text) Int) Int { 0 }',
      );
      harness.expectNoErrors();
      expect(
        harness.signatureOf('f'),
        'f(a: List[Int], b: (Int, Text) Int) Int',
      );
    });

    test('reports an unknown type', () {
      final harness = _lower('fun f(a: Nope) Int { 0 }');
      expect(harness.errorMessages, ['Unknown type: `Nope`.']);
    });

    test('reports a wrong type argument count', () {
      final harness = _lower('fun f(a: List[Int, Text]) Int { 0 }');
      expect(harness.errorMessages, ['Expected 1 type argument, got 2.']);
    });

    test('reports a trait used as a type', () {
      final harness = _lower('''
        trait ToText { fun toText(self: Self) Text }
        fun f(a: ToText) Int { 0 }
      ''');
      expect(harness.errorMessages, ["Traits can't be used as types."]);
    });

    test('reports `Self` outside a trait or impl', () {
      final harness = _lower('fun f(a: Self) Int { 0 }');
      expect(harness.errorMessages, [
        '`Self` can only be used in traits and impls.',
      ]);
    });

    test('reports duplicate parameter names', () {
      final harness = _lower('fun f(a: Int, a: Int) Int { 0 }');
      expect(harness.errorMessages, ['Duplicate parameter name: `a`.']);
    });
  });

  group('type parameters', () {
    test('keeps same-named parameters of different declarations apart', () {
      // The Rust compiler keys type parameters by name alone, so all three of
      // these `T`s are one key when building a substitution environment.
      final harness = _lower('''
        struct Box[T] { value: T }
        fun f[T](a: T) T { a }
        fun g[T](a: T) T { a }
      ''');
      harness.expectNoErrors();

      final box = harness.signatures.typeDeclarations.values
          .firstWhere((it) => it.id.name == 'Box')
          .typeParameters
          .single
          .id;
      final f = harness.signatures.functions.values
          .firstWhere((it) => it.signature.name == 'f')
          .signature
          .typeParameters
          .single
          .id;
      final g = harness.signatures.functions.values
          .firstWhere((it) => it.signature.name == 'g')
          .signature
          .typeParameters
          .single
          .id;

      expect({box, f, g}, hasLength(3));
      expect([box, f, g].map((it) => it.name), everyElement('T'));
    });

    test('rejects shadowing an enclosing type parameter', () {
      final harness = _lower('''
        trait Wrap[T] { fun wrap[T](self: Self, value: T) T }
      ''');
      expect(harness.errorMessages, ['Duplicate type parameter name: `T`.']);
    });

    test('substitutes only the parameters of the matching declaration', () {
      final harness = _lower('''
        struct Box[T] { value: T }
        fun f[T](a: T) T { a }
      ''');
      harness.expectNoErrors();

      final box = harness.signatures.typeDeclarations.values.firstWhere(
        (it) => it.id.name == 'Box',
      );
      final f = harness.signatures.functions.values
          .firstWhere((it) => it.signature.name == 'f')
          .signature;

      final field =
          (box.kind as HirTypeDeclarationKind_Struct).fields.single.type;
      // Substituting with `f`'s environment must leave `Box`'s `T` alone.
      expect(
        field.substitute({f.typeParameters.single.id: const HirType.error()}),
        field,
      );
      expect(
        field.substitute({box.typeParameters.single.id: const HirType.error()}),
        const HirType.error(),
      );
    });

    test('reports a duplicate type parameter name', () {
      final harness = _lower('fun f[T, T](a: Int) Int { 0 }');
      expect(harness.errorMessages, ['Duplicate type parameter name: `T`.']);
    });

    test('lowers an upper bound to a trait', () {
      final harness = _lower('''
        trait Compare { fun compareTo(self: Self, other: Self) Ordering }
        fun f[T: Compare](a: T) T { a }
      ''');
      harness.expectNoErrors();
      expect(harness.signatureOf('f'), 'f[T: Compare](a: T) T');
    });

    test('reports a type used as a bound', () {
      final harness = _lower('fun f[T: Int](a: T) T { a }');
      expect(harness.errorMessages, ["Types can't be used as traits."]);
    });
  });

  group('impls', () {
    test('matches impl functions to their trait functions', () {
      final harness = _lower('''
        trait ToText { fun toText(self: Self) Text }
        impl Int: ToText { fun toText(self: Int) Text { "int" } }
      ''');
      harness.expectNoErrors();

      final impl = harness.signatures.impls.single;
      expect(impl.functionsByTraitFunction, hasLength(1));

      final trait = harness.signatures.traits.values.single;
      expect(
        impl.functionsByTraitFunction.keys.single,
        trait.functionIds.single,
      );
    });

    test('reports a missing implementation', () {
      final harness = _lower('''
        trait ToText { fun toText(self: Self) Text }
        impl Int: ToText {}
      ''');
      expect(harness.errorMessages, [
        'Missing implementation of function toText(self: Int) Text.',
      ]);
    });

    test('accepts a trait function with a default implementation', () {
      final harness = _lower('''
        trait ToText { fun toText(self: Self) Text { "?" } }
        impl Int: ToText {}
      ''');
      expect(harness.errorMessages, isEmpty);
    });

    test('reports a function that is not part of the trait', () {
      final harness = _lower('''
        trait ToText { fun toText(self: Self) Text }
        impl Int: ToText {
          fun toText(self: Int) Text { "int" }
          fun extra(self: Int) Int { 0 }
        }
      ''');
      expect(harness.errorMessages, ['`extra` is not part of trait `ToText`.']);
    });

    test('reports a signature that does not match the trait', () {
      final harness = _lower('''
        trait ToText { fun toText(self: Self) Text }
        impl Int: ToText { fun toText(self: Int) Int { 0 } }
      ''');
      expect(harness.errorMessages.single, contains('Expected:'));
      expect(harness.errorMessages.single, contains('toText(self: Int) Text'));
    });

    test('records a solver rule for each impl', () {
      final harness = _lower('''
        trait Compare { fun compareTo(self: Self, other: Self) Ordering }
        trait Equal { fun equals(self: Self, other: Self) Bool }
        impl Ordering: Compare {
          fun compareTo(self: Ordering, other: Ordering) Ordering { other }
        }
        impl[T: Compare] T: Equal {
          fun equals(self: T, other: T) Bool { Bool.true_() }
        }
      ''');
      harness.expectNoErrors();

      final rules = harness.signatures.solverEnvironment.rules;
      expect(rules, hasLength(2));
      expect(
        rules.map((it) => '$it'),
        containsAll(<String>[
          'Compare(Ordering) <- <nothing>',
          'Equal(?T) <- Compare(?T)',
        ]),
      );
    });

    test("keeps a generic trait's type arguments in its solver rule", () {
      // The Rust compiler substitutes the trait's own type parameters here
      // instead of the impl's type arguments, so the rule it builds can never
      // unify with the trait's goal.
      final harness = _lower('''
        trait Into[T] { fun into(self: Self) T }
        impl Int: Into[Text] { fun into(self: Int) Text { "x" } }
      ''');
      harness.expectNoErrors();
      expect(
        harness.signatures.solverEnvironment.rules.single.toString(),
        'Into(Text, Int) <- <nothing>',
      );
    });
  });

  group('declarations', () {
    test('reports a duplicate type name', () {
      final harness = _lower('struct Foo {}\nenum Foo { a }');
      expect(harness.errorMessages, ['Duplicate type name: `Foo`.']);
    });

    test('reports a trait and struct sharing a name', () {
      final harness = _lower(
        'struct Foo {}\ntrait Foo { fun f(self: Self) Int }',
      );
      expect(harness.errorMessages, ['Duplicate type name: `Foo`.']);
    });

    test('allows overloaded top-level functions', () {
      final harness = _lower('''
        fun f(a: Int) Int { a }
        fun f(a: Text) Int { 0 }
      ''');
      expect(harness.errorMessages, isEmpty);
    });

    test('reports two overloads with the same parameter types', () {
      // The Rust compiler leaves this unchecked and only complains later, at
      // the call site, about the call being ambiguous.
      final harness = _lower('''
        fun f(a: Int) Int { a }
        fun f(b: Int) Text { "x" }
      ''');
      expect(harness.errorMessages, [
        'A function named `f` with the same parameter types already exists.',
      ]);
    });

    test('reports a function clashing with an assignment', () {
      final harness = _lower('''
        let f: Int = 1
        fun f(a: Int) Int { a }
      ''');
      expect(
        harness.errorMessages.single,
        "A top-level function can't have the same name as a top-level "
        'declaration: `f`.',
      );
    });
  });

  group('well-known types', () {
    test('finds the types the compiler needs by name', () {
      final harness = _lower('fun f(a: Int) Int { a }');
      final types = harness.signatures.wellKnownTypes;
      expect(types.intId, isNotNull);
      expect(types.textId, isNotNull);
      expect(types.neverId, isNotNull);
      expect(types.nothingId, isNotNull);
      expect('${types.maybeType(types.intType)}', 'Maybe[Int]');
    });

    test('degrades to an error type when one is missing', () {
      final harness = LoweringHarness.of('struct Int = builtin');
      expect('${harness.signatures.wellKnownTypes.textType}', '<error>');
    });
  });
}

LoweringHarness _lower(String source) => .of('$preludeSource\n$source');
