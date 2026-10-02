import 'package:test/test.dart';

import '../../mono/harness.dart';
import 'harness.dart';

void main() {
  group('declaring and binding', () {
    test('lowers a declaration and its binding', () {
      final harness = _lower('''
        $_iteratorSource
        impl Counter: Iterator {
          type Item = Int
          fun next(self: Counter) Int { self.value }
        }
      ''');
      harness.expectNoErrors();

      final trait = harness.signatures.traits.values
          .where((it) => it.id.name == 'Iterator')
          .single;
      expect(trait.associatedTypes.map((it) => '$it'), ['Item: ToText']);

      final impl = harness.signatures.impls.firstWhere(
        (it) => !it.isInherent && it.type.toString() == 'Counter',
      );
      expect(impl.associatedTypes.values.map((it) => '$it'), ['Int']);
    });

    test('resolves a projection in a trait function signature', () {
      final harness = _lower(_iteratorSource);
      harness.expectNoErrors();
      expect(
        harness.signatures.functions.values
            .where((it) => it.signature.name == 'next')
            .single
            .signature
            .toString(),
        'next(self: Self) Self.Item',
      );
    });

    // ignore: missing-test-assertion
    test('matches an impl function that writes the bound type out', () {
      // The impl says `Int` where the trait says `Self.Item`; normalizing both
      // is what makes them the same signature.
      final harness = _lower('''
        $_iteratorSource
        impl Counter: Iterator {
          type Item = Int
          fun next(self: Counter) Int { self.value }
        }
      ''');
      harness.expectNoErrors();
    });

    test('reports a missing binding', () {
      final harness = _lower('''
        $_iteratorSource
        impl Counter: Iterator {
          fun next(self: Counter) Int { self.value }
        }
      ''');
      expect(
        harness.errorMessages,
        contains('Missing binding for associated type `Item`.'),
      );
    });

    test('reports a binding the trait never declared', () {
      final harness = _lower('''
        $_iteratorSource
        impl Counter: Iterator {
          type Item = Int
          type Nope = Int
          fun next(self: Counter) Int { self.value }
        }
      ''');
      expect(harness.errorMessages, [
        '`Nope` is not an associated type of trait `Iterator`.',
      ]);
    });

    test('reports a duplicate binding', () {
      final harness = _lower('''
        $_iteratorSource
        impl Counter: Iterator {
          type Item = Int
          type Item = Text
          fun next(self: Counter) Int { self.value }
        }
      ''');
      expect(
        harness.errorMessages,
        contains('Duplicate associated type name: `Item`.'),
      );
    });

    test('reports a binding that violates the bound', () {
      final harness = _lower('''
        $_iteratorSource
        struct Opaque {}
        impl Counter: Iterator {
          type Item = Opaque
          fun next(self: Counter) Opaque { Opaque() }
        }
      ''');
      expect(harness.errorMessages, [
        // ignore: no_adjacent_strings_in_list
        '`Opaque` does not satisfy the bound `ToText` on associated type '
            '`Item`.',
      ]);
    });

    test('reports a value in a trait and a bound in an impl', () {
      final harness = _lower('''
        trait Weird { type Item = Int }
        impl Int: Weird { type Item: ToText }
      ''');
      expect(harness.errorMessages, [
        // ignore: no_adjacent_strings_in_list
        "A trait's associated type can't have a value; only an impl gives it "
            'one.',
        'Missing binding for associated type `Item`.',
        // ignore: no_adjacent_strings_in_list
        "An impl's associated type can't have a bound; only the trait declares "
            'one.',
        'This associated type is missing a value.',
      ]);
    });

    test('reports a binding on an impl without a trait', () {
      final harness = _lower('impl Int { type Item = Text }');
      expect(harness.errorMessages, [
        "An impl without a trait can't bind associated types.",
      ]);
    });

    test('reports a name shared with a function', () {
      final harness = _lower('trait T { type f fun f(self: Self) Int }');
      expect(harness.errorMessages, [
        'An associated type named `f` already exists.',
      ]);
    });
  });

  group('projections', () {
    test('reports an unknown associated type', () {
      final harness = _lower('''
        $_iteratorSource
        fun f[T: Iterator](it: T) T.Nope { it.next() }
      ''');
      expect(harness.errorMessages, [
        'Trait `Iterator` has no associated type `Nope`.',
      ]);
    });

    test('reports a projection off an unbounded type parameter', () {
      final harness = _lower('fun f[T](it: T) T.Item { it }');
      expect(harness.errorMessages, [
        '`T` has no associated types, because it has no bound.',
      ]);
    });

    test('reports a projection off a concrete type', () {
      final harness = _lower('''
        $_iteratorSource
        fun f() Int.Item { 0 }
      ''');
      expect(harness.errorMessages, [
        // ignore: no_adjacent_strings_in_list
        'Associated types can only be read off `Self` or a bounded type '
            'parameter.',
      ]);
    });

    test('reports `Self.Item` in an impl without a trait', () {
      final harness = _lower('impl Int { fun f(self: Int) Self.Item { 0 } }');
      expect(harness.errorMessages, [
        // ignore: no_adjacent_strings_in_list
        "`Self` has no associated types here, because this impl doesn't "
            'implement a trait.',
      ]);
    });

    test('reports an associated type used as a trait', () {
      final harness = _lower('''
        $_iteratorSource
        impl[T: Iterator] Counter: T.Item {}
      ''');
      expect(
        harness.errorMessages,
        contains('An associated type is not a trait.'),
      );
    });
  });

  group('inference', () {
    test('resolves a projection through a concrete impl', () {
      final harness = _lowerFull('''
        $_iteratorSource
        $_counterImplSource
        fun f() Text { Counter(1).next().toText() }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
      expect(harness.bindingsOf('f').values, isNot(contains('Counter.Item')));
      expect(harness.callsIn('f'), [
        r'Core.test:$impl#1.next',
        r'Core.test:$impl.toText',
      ]);
    });

    test('keeps a projection rigid through a type parameter', () {
      final harness = _lowerFull('''
        $_iteratorSource
        $_counterImplSource
        fun describe[T: Iterator](it: T) Text { it.next().toText() }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
      // `toText` is only callable because the trait bounds `Item` by `ToText`.
      expect(harness.bodyTextOf('describe'), contains('T.Item'));
      expect(harness.callsIn('describe'), [
        'Core.test:Iterator.next',
        'Core.test:ToText.toText',
      ]);
    });

    test('rejects a method the bound does not provide', () {
      final harness = _lowerFull('''
        $_iteratorSource
        trait Double { fun double(self: Self) Self }
        fun describe[T: Iterator](it: T) Text { it.next().double() }
        fun main() Int { 0 }
      ''');
      expect(
        harness.errorMessages,
        contains(startsWith('No function `double` matches this call')),
      );
    });

    test('offers no methods when the associated type has no bound', () {
      final harness = _lowerFull('''
        trait Holder {
          type Item
          fun get(self: Self) Self.Item
        }
        impl Int: Holder { type Item = Int fun get(self: Int) Int { self } }
        fun describe[T: Holder](it: T) Int { it.get().toText() }
        fun main() Int { 0 }
      ''');
      expect(
        harness.errorMessages,
        contains(startsWith('No function `toText` matches this call')),
      );
    });

    test('resolves a projection through a generic impl', () {
      final harness = _lowerFull('''
        $_iteratorSource
        struct Wrapper[T] { value: T }
        impl[T: ToText] Wrapper[T]: Iterator {
          type Item = T
          fun next(self: Wrapper[T]) T { self.value }
        }
        fun f() Text { Wrapper(1).next().toText() }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
      expect(harness.bindingsOf('f').values, isNot(contains('Wrapper.Item')));
    });

    // ignore: missing-test-assertion
    test("resolves `Self`'s projection in a blanket impl to its binding", () {
      final harness = _lowerFull('''
        trait Source {
          type Value
          fun get(self: Self) Self.Value
        }
        trait Typed {
          type Value
          fun read(self: Self) Self.Value
          fun echo(self: Self, value: Self.Value) Self.Value
        }
        impl[T: Source] T: Typed {
          type Value = T.Value
          fun read(self: Self) Self.Value { self.get() }
          fun echo(self: Self, value: Self.Value) Self.Value {
            pass(self, value)
          }
        }
        fun pass[T: Source](it: T, value: T.Value) T.Value { value }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
    });

    // ignore: missing-test-assertion
    test('reads a field through a projection the impl binds', () {
      final harness = _lowerFull('''
        trait Source {
          type Value
          fun get(self: Self) Self.Value
        }
        struct Box[T] { inner: T }
        trait Boxed {
          type Value
          fun unbox(self: Self, value: Self.Value) Int
        }
        impl[T: Source] T: Boxed {
          type Value = Box[Int]
          fun unbox(self: Self, value: Self.Value) Int { value.inner }
        }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
    });
  });

  group('monomorphization', () {
    test("resolves the projection to the impl's binding", () {
      final harness = MonoHarness.of('''
        $_iteratorSource
        $_counterImplSource
        fun describe[T: Iterator](it: T) Text { it.next().toText() }
        fun main() Int { builtinPrint(describe(Counter(41))) 0 }
      ''');
      expect(harness.instancesOf('describe'), ['describe[Counter]']);
      expect(
        describeType(harness.instance('describe[Counter]').returnType),
        'Text',
      );
      // Both calls are statically dispatched: `next` to the `Counter` impl and
      // `toText` to the `Int` impl, which is what `Item` binds to.
      expect(harness.callsIn('describe[Counter]'), [
        r'$impl#1.next',
        r'$impl.toText',
      ]);
    });
  });
}

LoweringHarness _lower(String source) => .of('$preludeSource\n$source');
LoweringHarness _lowerFull(String source) => .full('$preludeSource\n$source');

const _iteratorSource = '''
trait ToText { fun toText(self: Self) Text }
impl Int: ToText { fun toText(self: Int) Text { builtinIntToText(self) } }

trait Iterator {
  type Item: ToText
  fun next(self: Self) Self.Item
}

struct Counter { value: Int }
''';

const _counterImplSource = '''
impl Counter: Iterator {
  type Item = Int
  fun next(self: Counter) Int { self.value }
}
''';
