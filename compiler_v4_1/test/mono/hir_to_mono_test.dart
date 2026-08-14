// ignore_for_file: missing-test-assertion

import 'package:test/test.dart';

import 'harness.dart';

void main() {
  group('generics', () {
    test('instantiates once per type argument', () {
      final harness = MonoHarness.of('''
        fun identity[T](value: T) T { value }
        fun main() Int {
          identity(1)
          identity("a")
          0
        }
      ''');
      expect(harness.instancesOf('identity'), [
        'identity[Int]',
        'identity[Text]',
      ]);
    });

    test('shares one instantiation between call sites', () {
      final harness = MonoHarness.of('''
        fun identity[T](value: T) T { value }
        fun main() Int {
          identity(1)
          identity(2)
          0
        }
      ''');
      expect(harness.instancesOf('identity'), ['identity[Int]']);
    });

    test('passes type arguments on to nested calls', () {
      final harness = MonoHarness.of('''
        fun inner[T](value: T) T { value }
        fun outer[T](value: T) T { inner(value) }
        fun main() Int {
          outer("a")
          0
        }
      ''');
      expect(harness.callsIn('outer[Text]'), ['inner[Text]']);
    });

    test('monomorphizes a generic type per argument', () {
      final harness = MonoHarness.of('''
        fun wrap[T](value: T) Maybe[T] { Maybe.some(value) }
        fun main() Int {
          wrap(1)
          wrap("a")
          0
        }
      ''');
      expect(harness.typeNames, containsAll(['Maybe[Int]', 'Maybe[Text]']));
    });

    test('terminates on a recursive function', () {
      final harness = MonoHarness.of('''
        fun repeat[T](value: T) T { repeat(value) }
        fun main() Int { repeat(1) }
      ''');
      expect(harness.callsIn('repeat[Int]'), ['repeat[Int]']);
    });

    test('terminates on a recursive type', () {
      final harness = MonoHarness.of('''
        struct Node { next: Maybe[Node] }
        fun main() Int {
          Node(Maybe.none())
          0
        }
      ''');
      expect(harness.typeNames, containsAll(['Node', 'Maybe[Node]']));
    });
  });

  group('trait dispatch', () {
    const toText = '''
      trait ToText {
        fun toText(self: Self) Text
      }
      impl Int: ToText {
        fun toText(self: Int) Text { "int" }
      }
    ''';

    test('calls the impl for a concrete receiver', () {
      final harness = MonoHarness.of('''
        $toText
        fun main() Int {
          1.toText()
          0
        }
      ''');
      expect(harness.callsIn('main'), [r'$impl.toText']);
    });

    test('resolves a call through a bounded type parameter', () {
      final harness = MonoHarness.of('''
        $toText
        fun describe[T: ToText](value: T) Text { value.toText() }
        fun main() Int {
          describe(1)
          0
        }
      ''');
      expect(harness.callsIn('describe[Int]'), [r'$impl.toText']);
    });

    test('instantiates a generic impl per receiver type', () {
      final harness = MonoHarness.of('''
        trait Wrapped {
          fun unwrap(self: Self) Nothing
        }
        impl[T] Maybe[T]: Wrapped {
          fun unwrap(self: Maybe[T]) Nothing { Nothing() }
        }
        fun main() Int {
          Maybe.some(1).unwrap()
          Maybe.some("a").unwrap()
          0
        }
      ''');
      expect(harness.instancesOf('unwrap'), [
        r'$impl.unwrap[Int]',
        r'$impl.unwrap[Text]',
      ]);
    });

    test('falls back to a default trait body', () {
      final harness = MonoHarness.of('''
        trait Greet {
          fun greet(self: Self) Text { "hi" }
        }
        impl Int: Greet {}
        fun main() Int {
          1.greet()
          0
        }
      ''');
      expect(harness.instancesOf('greet'), ['Greet.greet[Int]']);
    });

    // Overload resolution drops the blanket candidate here, so the call is
    // left pointing at the trait function and only monomorphization has to
    // decide between the two impls.
    test('prefers an impl for the concrete type over a blanket one', () {
      final harness = MonoHarness.of('''
        trait Greet {
          fun greet(self: Self) Text { "hi" }
        }
        impl Int: Greet {}
        impl[T] T: Greet {
          fun greet(self: T) Text { "blanket" }
        }
        fun main() Int {
          1.greet()
          0
        }
      ''');
      expect(harness.instancesOf('greet'), ['Greet.greet[Int]']);
    });

    test('instantiates a blanket impl for the receiver', () {
      final harness = MonoHarness.of('''
        trait Named {
          fun name(self: Self) Text
        }
        trait Labeled {
          fun label(self: Self) Text
        }
        impl[T: Named] T: Labeled {
          fun label(self: T) Text { self.name() }
        }
        impl Int: Named {
          fun name(self: Int) Text { "int" }
        }
        fun describe[T: Labeled](value: T) Text { value.label() }
        fun main() Int {
          describe(1)
          0
        }
      ''');
      expect(harness.instancesOf('label'), [r'$impl.label[Int]']);
      expect(harness.callsIn(r'$impl.label[Int]'), [r'$impl#1.name']);
    });
  });

  group('inherent impls', () {
    test('calls a method declared without a trait', () {
      final harness = MonoHarness.of('''
        impl Int {
          fun double(self: Int) Int { builtinIntAdd(self, self) }
        }
        fun main() Int {
          1.double()
        }
      ''');
      expect(harness.callsIn('main'), [r'$impl.double']);
    });

    test('instantiates a generic inherent impl per receiver type', () {
      final harness = MonoHarness.of('''
        impl[T] Maybe[T] {
          fun orElse(self: Maybe[T], fallback: T) T { fallback }
        }
        fun main() Int {
          Maybe.some("a").orElse("b")
          Maybe.some(1).orElse(2)
        }
      ''');
      expect(harness.instancesOf('orElse'), [
        r'$impl.orElse[Int]',
        r'$impl.orElse[Text]',
      ]);
    });
  });

  group('bodies', () {
    test('gives a switch case binding the variant payload type', () {
      final harness = MonoHarness.of('''
        fun unwrapOr(value: Maybe[Int], fallback: Int) Int {
          switch value {
            some(inner) => inner,
            none => fallback,
          }
        }
        fun main() Int { unwrapOr(Maybe.some(1), 0) }
      ''');

      final switch_ = harness.switchIn('unwrapOr');
      expect(describeType(switch_.enumType), 'Maybe[Int]');
      expect(
        switch_.cases
            .map(
              (it) => it.value == null
                  ? it.variant
                  : '${it.variant}: ${describeType(it.value!.$2)}',
            )
            .toList(),
        ['some: Int', 'none'],
      );
    });

    test('monomorphizes a lambda and its type', () {
      final harness = MonoHarness.of('''
        fun apply[T](value: T, function: (T) T) T { function(value) }
        fun main() Int { apply(1, (it: Int) { it }) }
      ''');
      expect(harness.instancesOf('apply'), ['apply[Int]']);
      expect(harness.typeNames, contains('(Int) Int'));
    });

    test('lowers builtins with their type arguments', () {
      final harness = MonoHarness.of('''
        fun main() Int {
          builtinListOf(1)
          0
        }
      ''');
      expect(harness.instancesOf('builtinListOf'), ['builtinListOf[Int]']);
      expect(harness.typeNames, contains('List[Int]'));
    });
  });

  group('assignments', () {
    test('initializes an assignment before the one that reads it', () {
      final harness = MonoHarness.of('''
        let a: Int = b
        let b: Int = 1
        fun main() Int { a }
      ''');
      expect(harness.assignmentOrder, ['b', 'a']);
    });

    test('leaves out assignments nothing reaches', () {
      final harness = MonoHarness.of('''
        let used: Int = 1
        let unused: Int = 2
        fun main() Int { used }
      ''');
      expect(harness.assignmentOrder, ['used']);
    });
  });

  test('leaves out functions `main` never reaches', () {
    final harness = MonoHarness.of('''
      fun unused() Int { 1 }
      fun main() Int { 0 }
    ''');
    expect(harness.instancesOf('unused'), isEmpty);
  });
}
