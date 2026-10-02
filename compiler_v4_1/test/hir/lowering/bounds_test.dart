import 'package:test/test.dart';

import '../../mono/harness.dart';
import 'harness.dart';

void main() {
  group('type parameters', () {
    test('lowers every bound', () {
      final harness = _lower('''
        $_traitsSource
        fun f[T: ToText + Compare](a: T) Text { a.toText() }
      ''');
      harness.expectNoErrors();
      expect(harness.signatureOf('f'), 'f[T: ToText + Compare](a: T) Text');
    });

    // ignore: missing-test-assertion
    test('offers the functions of every bound', () {
      final harness = _lowerFull('''
        $_traitsSource
        fun f[T: ToText + Compare](a: T, b: T) Text {
          a.compareTo(b)
          a.toText()
        }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
    });

    test('requires every bound at the call site', () {
      final harness = _lowerFull('''
        $_traitsSource
        fun f[T: ToText + Compare](a: T) Text { a.toText() }
        fun main() Int { f(1) 0 }
      ''');
      expect(
        harness.errorMessages.single,
        contains('the bound `Compare(?T)` is not satisfied'),
      );
    });

    // ignore: missing-test-assertion
    test('accepts a type that satisfies every bound', () {
      final harness = _lowerFull('''
        $_traitsSource
        impl Int: Compare {
          fun compareTo(self: Int, other: Int) Ordering { Ordering.equal() }
        }
        fun f[T: ToText + Compare](a: T) Text { a.toText() }
        fun main() Int { f(1) 0 }
      ''');
      harness.expectNoErrors();
    });

    test('keeps a duplicate bound only once', () {
      final harness = _lower('''
        $_traitsSource
        fun f[T: ToText + ToText](a: T) Text { a.toText() }
      ''');
      harness.expectNoErrors();
      expect(harness.signatureOf('f'), 'f[T: ToText](a: T) Text');
    });

    test('lowers a later bound against the earlier type parameters', () {
      final harness = _lower('''
        $_traitsSource
        trait Into[T] { fun into(self: Self) T }
        fun f[T, U: ToText + Into[T]](a: U) Text { a.toText() }
      ''');
      harness.expectNoErrors();
      expect(harness.signatureOf('f'), 'f[T, U: ToText + Into[T]](a: U) Text');
    });

    test('reports a bound that is not a trait', () {
      final harness = _lower('''
        $_traitsSource
        fun f[T: ToText + Int](a: T) Text { a.toText() }
      ''');
      expect(harness.errorMessages.single, "Types can't be used as traits.");
    });
  });

  group('associated types', () {
    test('lowers every bound', () {
      final harness = _lower('''
        $_traitsSource
        trait Iterator {
          type Item: ToText + Compare
          fun next(self: Self) Self.Item
        }
      ''');
      harness.expectNoErrors();

      final trait = harness.signatures.traits.values
          .where((it) => it.id.name == 'Iterator')
          .single;
      expect(trait.associatedTypes.map((it) => '$it'), [
        'Item: ToText + Compare',
      ]);
    });

    test("checks every bound against an impl's binding", () {
      final harness = _lower('''
        $_traitsSource
        $_iteratorSource
        impl Counter: Iterator {
          type Item = Int
          fun next(self: Counter) Int { self.value }
        }
      ''');
      expect(
        harness.errorMessages.single,
        '`Int` does not satisfy the bound `Compare` on associated type `Item`.',
      );
    });

    // ignore: missing-test-assertion
    test('offers the functions of every bound on a projection', () {
      final harness = _lowerFull('''
        $_traitsSource
        impl Int: Compare {
          fun compareTo(self: Int, other: Int) Ordering { Ordering.equal() }
        }
        $_iteratorSource
        fun f[T: Iterator](it: T, other: T.Item) Text {
          it.next().compareTo(other)
          it.next().toText()
        }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
    });

    test('resolves a projection to the bound that declares it', () {
      final harness = _lower('''
        $_traitsSource
        $_iteratorSource
        trait Keyed { type Key fun key(self: Self) Self.Key }
        fun f[T: Keyed + Iterator](it: T) T.Item { it.next() }
      ''');
      harness.expectNoErrors();
      expect(harness.signatureOf('f'), 'f[T: Keyed + Iterator](it: T) T.Item');
    });

    test('reports a projection that no bound declares', () {
      final harness = _lower('''
        $_traitsSource
        $_iteratorSource
        trait Keyed { type Key fun key(self: Self) Self.Key }
        fun f[T: Keyed + Iterator](it: T) T.Value { it.next() }
      ''');
      expect(
        harness.errorMessages,
        contains('None of the bounds on `T` has an associated type `Value`.'),
      );
    });

    test('reports a projection that two bounds declare', () {
      final harness = _lower('''
        $_traitsSource
        $_iteratorSource
        trait Holder { type Item fun get(self: Self) Self.Item }
        fun f[T: Holder + Iterator](it: T) T.Item { it.next() }
      ''');
      expect(
        harness.errorMessages,
        contains(
          'Ambiguous associated type `Item`: declared by `Holder`, `Iterator`.',
        ),
      );
    });
  });

  group('monomorphization', () {
    test('dispatches the calls of both bounds to their impls', () {
      final harness = MonoHarness.of('''
        $_traitsSource
        impl Int: Compare {
          fun compareTo(self: Int, other: Int) Ordering { Ordering.equal() }
        }
        fun describe[T: ToText + Compare](a: T, b: T) Text {
          a.compareTo(b)
          a.toText()
        }
        fun main() Int { builtinPrint(describe(1, 2)) 0 }
      ''');
      expect(harness.instancesOf('describe'), ['describe[Int]']);
      expect(harness.callsIn('describe[Int]'), [
        r'$impl#1.compareTo',
        r'$impl.toText',
      ]);
    });
  });

  group('parsing', () {
    test('reports a missing trait after the plus sign', () {
      final harness = LoweringHarness.parseOnly(
        'fun f[T: ToText + ](a: T) Int { 0 }',
      );
      expect(
        harness.errorMessages.single,
        'This bound is missing a trait after the plus sign.',
      );
    });
  });
}

LoweringHarness _lower(String source) => .of('$preludeSource\n$source');
LoweringHarness _lowerFull(String source) => .full('$preludeSource\n$source');

const _traitsSource = '''
trait ToText { fun toText(self: Self) Text }
trait Compare { fun compareTo(self: Self, other: Self) Ordering }
impl Int: ToText { fun toText(self: Int) Text { builtinIntToText(self) } }
''';

const _iteratorSource = '''
trait Iterator {
  type Item: ToText + Compare
  fun next(self: Self) Self.Item
}
struct Counter { value: Int }
''';
