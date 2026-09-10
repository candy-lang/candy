import 'package:test/test.dart';

import 'harness.dart';

void main() {
  group('inherent impls', () {
    test('resolves a method on a concrete type', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun double(self: Int) Int { self }
        }
        fun main() Int {
          1.double()
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl.double']);
    });

    test('resolves a method on a generic type', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl[T] Maybe[T] {
          fun orElse(self: Maybe[T], fallback: T) T { fallback }
        }
        fun main() Int {
          Maybe.some(1).orElse(2)
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl.orElse']);
    });

    test('lets `Self` stand for the impl type', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun identity(self: Self) Self { self }
        }
        fun main() Int {
          1.identity()
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl.identity']);
    });

    test('reports a method the impl type does not have', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun double(self: Int) Int { self }
        }
        fun main() Int {
          "a".double()
          0
        }
      ''');
      expect(
        harness.errorMessages.join('\n'),
        contains('No function `double` matches this call'),
      );
    });

    test('honors a bound on the impl type parameter', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        trait Named {
          fun name(self: Self) Text
        }
        impl Int: Named {
          fun name(self: Int) Text { "int" }
        }
        impl[T: Named] Maybe[T] {
          fun describe(self: Maybe[T]) Text { "maybe" }
        }
        fun main() Int {
          Maybe.some(1).describe()
          0
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl#1.describe']);
    });

    test('rejects a receiver that fails the impl bound', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        trait Named {
          fun name(self: Self) Text
        }
        impl[T: Named] Maybe[T] {
          fun describe(self: Maybe[T]) Text { "maybe" }
        }
        fun main() Int {
          Maybe.some(1).describe()
          0
        }
      ''');
      expect(
        harness.errorMessages.join('\n'),
        contains('the bound `Named(Int)` is not satisfied'),
      );
    });

    test('rejects an impl without a trait on a type parameter', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl[T] T {
          fun anything(self: T) Int { 0 }
        }
        fun main() Int { 0 }
      ''');
      expect(
        harness.errorMessages,
        contains('An impl without a trait needs a concrete type.'),
      );
    });

    test('reports two inherent methods with the same signature', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun twin(self: Int) Int { self }
        }
        impl Int {
          fun twin(self: Int) Int { self }
        }
        fun main() Int {
          1.twin()
        }
      ''');
      expect(
        harness.errorMessages.join('\n'),
        contains('Multiple functions match this call'),
      );
    });

    test('prefers an inherent method over a trait one', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        trait Doubled {
          fun double(self: Self) Int
        }
        impl Int: Doubled {
          fun double(self: Int) Int { 1 }
        }
        impl Int {
          fun double(self: Int) Int { 2 }
        }
        fun main() Int {
          1.double()
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl#1.double']);
    });

    test('calls a static function on the impl type', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun parse(text: Text) Int { 0 }
        }
        fun main() Int {
          Int.parse("1")
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl.parse']);
    });

    test('infers the impl type parameters of a static function', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl[T] Maybe[T] {
          fun empty() Maybe[T] { Maybe.none() }
        }
        fun main() Int {
          let value: Maybe[Int] = Maybe.empty()
          0
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl.empty']);
    });

    test('checks the return type of a static function against its context', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl[T] Maybe[T] {
          fun empty() Maybe[T] { Maybe.none() }
        }
        fun main() Int {
          let value: Int = Maybe.empty()
          0
        }
      ''');
      expect(
        harness.errorMessages.join('\n'),
        contains('Expected type `Int`, got `Maybe['),
      );
    });

    test('lets a static function return `Self`', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun zero() Self { 0 }
        }
        fun main() Int {
          Int.zero()
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl.zero']);
    });

    test('overloads a static function on its arguments', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun of(text: Text) Int { 0 }
          fun of(value: Int) Int { value }
        }
        fun main() Int {
          Int.of("1")
          Int.of(2)
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [
        r'Core.test:$impl.of',
        r'Core.test:$impl.of#1',
      ]);
    });

    test('lets a method also be called statically', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun double(self: Int) Int { self }
        }
        fun main() Int {
          Int.double(1)
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [r'Core.test:$impl.double']);
    });

    test('prefers an enum variant over a static function', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl[T] Maybe[T] {
          fun none() Int { 0 }
        }
        fun main() Int {
          let value: Maybe[Int] = Maybe.none()
          0
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), isEmpty);
    });

    test('reports a static function that is not visible', () {
      final harness = LoweringHarness.fullPackages({
        'core': {
          'int': '''
            public struct Int = builtin
            public struct Text = builtin
            impl Int { fun parse(text: Text) Int { 0 } }
          ''',
        },
        'app': {
          'main': '''
            use core.int
            fun main() Int { Int.parse("1") }
          ''',
        },
      });
      expect(
        harness.errorMessages.join('\n'),
        contains('The function `parse` is private.'),
      );
    });

    test('reports a static function referenced without a call', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun parse(text: Text) Int { 0 }
        }
        fun main() Int {
          Int.parse
        }
      ''');
      expect(
        harness.errorMessages.join('\n'),
        contains('Referencing a function as a value is not supported yet.'),
      );
    });

    test('reports a static call with no matching function', () {
      final harness = LoweringHarness.full('''
        $preludeSource
        impl Int {
          fun parse(text: Text) Int { 0 }
        }
        fun main() Int {
          Int.parse(1)
        }
      ''');
      expect(
        harness.errorMessages.join('\n'),
        contains('No function matches this call'),
      );
    });
  });
}
