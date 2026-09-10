// ignore_for_file: missing-test-assertion

import 'package:test/test.dart';

import 'harness.dart';

void main() {
  group('expressions', () {
    test('lowers literals and builtin calls', () {
      final harness = _lower('''
        fun main() Int {
          builtinPrint(builtinIntToText(42))
          0
        }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main'), [
        'Builtins:builtinIntToText',
        'Builtins:builtinPrint',
      ]);
    });

    test('folds text interpolation into concatenations', () {
      final harness = _lower('''
        fun main() Int {
          builtinPrint("a{"b"}c")
          0
        }
      ''');
      harness.expectNoErrors();
      expect(
        harness.callsIn('main').where((it) => it.contains('TextConcat')),
        hasLength(2),
      );
    });

    test('reports an unknown reference', () {
      final harness = _lower('fun main() Int { nope }');
      expect(harness.errorMessages, ['Unknown reference: `nope`.']);
    });

    test('checks the declared type of a binding', () {
      final harness = _lower('fun main() Int { let a: Text = 1 \n 0 }');
      expect(harness.errorMessages, ['Expected type `Text`, got `Int`.']);
    });

    test('accesses a struct field', () {
      final harness = _lower('''
        struct Box { value: Int }
        fun main() Int { Box(1).value }
      ''');
      harness.expectNoErrors();
    });

    test('reports an unknown field', () {
      final harness = _lower('''
        struct Box { value: Int }
        fun main() Int { Box(1).nope }
      ''');
      expect(harness.errorMessages, [
        'A value of type `Box` has no field `nope`.',
      ]);
    });

    test('reports a non-exhaustive switch', () {
      // The Rust compiler leaves this unchecked.
      final harness = _lower('''
        fun main() Int {
          switch Ordering.less() { less => 0, equal => 1 }
        }
      ''');
      expect(harness.errorMessages, ['Switch is missing variant: `greater`.']);
    });

    test('accesses a struct field through `Self`', () {
      final harness = _lower('''
        struct Box { value: Int }
        impl Box {
          fun get(self: Self) Int { self.value }
        }
        fun main() Int { Box(1).get() }
      ''');
      harness.expectNoErrors();
    });

    test('switches over a value of type `Self`', () {
      final harness = _lower('''
        impl Ordering {
          fun toInt(self: Self) Int {
            switch self { less => 0, equal => 1, greater => 2 }
          }
        }
        fun main() Int { Ordering.less().toInt() }
      ''');
      harness.expectNoErrors();
    });

    test('binds a switch case payload', () {
      final harness = _lower('''
        fun main() Int {
          switch Maybe.some(1) { none => 0, some(value) => value }
        }
      ''');
      harness.expectNoErrors();
    });
  });

  group('inference', () {
    test('infers a binding type from its initializer', () {
      final harness = _lower('fun main() Int { let a = 1 \n a }');
      harness.expectNoErrors();
      expect(harness.bindingsOf('main'), {'a': 'Int'});
    });

    test('infers a type argument from a later annotated use', () {
      // The whole point of deferring: `Maybe.none()` alone says nothing about
      // `T`, and the annotation two statements later settles it.
      final harness = _lower('''
        fun main() Int {
          let a = Maybe.none()
          let b: Maybe[Int] = a
          0
        }
      ''');
      harness.expectNoErrors();
      expect(harness.bindingsOf('main'), {
        'a': 'Maybe[Int]',
        'b': 'Maybe[Int]',
      });
    });

    test('infers a type argument from a later call argument', () {
      final harness = _lower('''
        fun takesText(a: Maybe[Text]) Int { 0 }
        fun main() Int {
          let a = Maybe.none()
          takesText(a)
        }
      ''');
      harness.expectNoErrors();
      expect(harness.bindingsOf('main')['a'], 'Maybe[Text]');
    });

    test('reports a binding whose type never gets pinned down', () {
      final harness = _lower('''
        fun main() Int {
          let a = Maybe.none()
          0
        }
      ''');
      expect(harness.errorMessages, [
        "The type of `a` can't be inferred. Add a type annotation.",
      ]);
    });

    test('reports conflicting uses of one binding', () {
      final harness = _lower('''
        fun takesText(a: Maybe[Text]) Int { 0 }
        fun main() Int {
          let a: Maybe[Int] = Maybe.none()
          takesText(a)
        }
      ''');
      expect(harness.errorMessages.single, contains('Maybe[Int]'));
    });

    test('checks a lambda against the expected function type', () {
      final harness = _lower('''
        fun apply(f: (Int) Int) Int { 0 }
        fun main() Int { apply((x: Int) { x }) }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main').single, endsWith(':apply'));
    });

    test('infers an un-annotated lambda parameter from the callee', () {
      final harness = _lower('''
        fun apply(f: (Int) Int) Int { 0 }
        fun main() Int { apply((x) { x }) }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main').single, endsWith(':apply'));
    });

    test('infers un-annotated parameters of a multi-parameter lambda', () {
      final harness = _lower('''
        fun apply(f: (Int, Text) Text) Int { 0 }
        fun main() Int { apply((a, b) { b }) }
      ''');
      harness.expectNoErrors();
    });

    test('allows mixing annotated and un-annotated parameters', () {
      final harness = _lower('''
        fun apply(f: (Int, Text) Text) Int { 0 }
        fun main() Int { apply((a: Int, b) { b }) }
      ''');
      harness.expectNoErrors();
    });

    test('resolves member calls on an inferred lambda parameter', () {
      // `x`'s type is only known once `apply` resolves, which happens after
      // the lambda's own body has been lowered.
      final harness = _lower('''
        $_traitsSource
        fun apply(f: (Int) Text) Int { 0 }
        fun main() Int { apply((x) { x.toText() }) }
      ''');
      harness.expectNoErrors();
    });

    test('takes the parameter type from an annotated binding', () {
      final harness = _lower('''
        fun main() Int {
          let f: (Int) Int = (x) { x }
          0
        }
      ''');
      harness.expectNoErrors();
    });

    test('reports an un-annotated lambda parameter it cannot infer', () {
      final harness = _lower('''
        fun main() Int {
          let f = (x) { 0 }
          0
        }
      ''');
      expect(
        harness.errorMessages,
        contains("The type of `x` can't be inferred. Add a type annotation."),
      );
    });

    test('still requires annotations on function parameters', () {
      final harness = LoweringHarness.parseOnly('fun f(a) Int { 0 }');
      expect(harness.errorMessages, [
        'This parameter is missing a colon.',
        'This parameter is missing a type.',
      ]);
    });

    test('reports a lambda whose type does not match', () {
      final harness = _lower('''
        fun apply(f: (Int) Int) Int { 0 }
        fun main() Int { apply((x: Text) { 1 }) }
      ''');
      expect(harness.errorMessages.single, contains('No function matches'));
    });
  });

  group('overload resolution', () {
    test('picks the overload matching the argument types', () {
      final harness = _lower('''
        fun f(a: Int) Int { a }
        fun f(a: Text) Int { 0 }
        fun main() Int { f(1) }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main').single, endsWith(':f'));
    });

    test('prefers the more specific of two applicable overloads', () {
      // Both apply to an `Int`; the concrete one wins, as in Kotlin. The Rust
      // compiler rejects this call as ambiguous instead.
      final harness = _lower('''
        $_traitsSource
        fun f(a: Int) Int { a }
        fun f[T: ToText](a: T) Int { 0 }
        fun main() Int { f(1) }
      ''');
      harness.expectNoErrors();
      expect(harness.callsIn('main').single, 'Core.test:f');
    });

    test('reports genuinely incomparable candidates as ambiguous', () {
      final harness = _lower('''
        struct Box[A, B] { a: A, b: B }
        fun f[A](a: Box[A, Int]) Int { 0 }
        fun f[B](a: Box[Int, B]) Int { 0 }
        fun main() Int { f(Box(1, 1)) }
      ''');
      expect(harness.errorMessages.single, startsWith('Multiple functions'));
      expect(harness.errorMessages.single, contains('Box[A, Int]'));
      expect(harness.errorMessages.single, contains('Box[Int, B]'));
    });

    test('lists candidates when nothing matches', () {
      final harness = _lower('''
        fun f(a: Int) Int { a }
        fun main() Int { f("x") }
      ''');
      expect(harness.errorMessages.single, contains('No function matches'));
      expect(harness.errorMessages.single, contains('expected `Int` for `a`'));
    });

    test('reports a wrong argument count', () {
      final harness = _lower('''
        fun f(a: Int) Int { a }
        fun main() Int { f(1, 2) }
      ''');
      expect(
        harness.errorMessages.single,
        contains('wrong number of arguments'),
      );
    });
  });

  group('member calls', () {
    test('dispatches to an impl of a trait', () {
      final harness = _lower('''
        $_traitsSource
        fun main() Int {
          builtinPrint(1.toText())
          0
        }
      ''');
      harness.expectNoErrors();
    });

    test('dispatches through a blanket impl', () {
      final harness = _lower('''
        $_traitsSource
        fun main() Int {
          switch Ordering.less().equals(Ordering.greater()) {
            false_ => 0,
            true_ => 1,
          }
        }
      ''');
      harness.expectNoErrors();
    });

    test('dispatches on a type parameter through its bound', () {
      final harness = _lower('''
        $_traitsSource
        fun f[T: Compare](a: T, b: T) Ordering { a.compareTo(b) }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
    });

    test('rejects a member call on an unbounded type parameter', () {
      final harness = _lower('''
        $_traitsSource
        fun f[T](a: T, b: T) Ordering { a.compareTo(b) }
        fun main() Int { 0 }
      ''');
      expect(harness.errorMessages.single, contains('No function `compareTo`'));
    });

    test('satisfies a bound through `Self` in an inherent impl', () {
      final harness = _lower('''
        $_traitsSource
        fun isEqual[T: Equal](left: T, right: T) Bool { left.equals(right) }
        impl Ordering {
          fun isLess(self: Self) Bool { isEqual(self, Ordering.less()) }
        }
        fun main() Int { 0 }
      ''');
      harness.expectNoErrors();
    });

    test('names the impls a bound could be satisfied by', () {
      final harness = _lower('''
        $_traitsSource
        impl[T] T: Compare {
          fun compareTo(self: T, other: T) Ordering { Ordering.less() }
        }
        fun isBigger[T: Compare](left: T, right: T) Bool { Bool.true_() }
        fun main() Int {
          isBigger(Ordering.less(), Ordering.greater())
          0
        }
      ''');
      expect(harness.errorMessages.single, contains('matches several impls'));
      expect(
        harness.errorMessages.single,
        contains('impl `Compare(Ordering)`'),
      );
      expect(harness.errorMessages.single, contains('impl `Compare(?T)`'));
    });

    test('does not find top-level functions in method position', () {
      // UFCS is gone: `builtinIntToText` is a function, not a member of `Int`.
      final harness = _lower(
        'fun main() Int { builtinPrint(42.builtinIntToText()) \n 0 }',
      );
      expect(
        harness.errorMessages.first,
        contains('No function `builtinIntToText`'),
      );
    });
  });

  group('top-level assignments', () {
    test('infers the type of an un-annotated assignment', () {
      // The Rust compiler rejects this outright with "Assignment is missing a
      // type".
      final harness = _lower('''
        let answer = 42
        fun main() Int { answer }
      ''');
      harness.expectNoErrors();
      expect('${harness.hir.assignments.values.single.type}', 'Int');
    });

    test('lowers assignments in dependency order', () {
      final harness = _lower('''
        let b = a
        let a = 42
        fun main() Int { b }
      ''');
      harness.expectNoErrors();
      expect(
        harness.hir.assignments.values.map((it) => '${it.name}: ${it.type}'),
        containsAll(<String>['a: Int', 'b: Int']),
      );
    });

    test('reports a cycle between assignments', () {
      final harness = _lower('''
        let a = b
        let b = a
        fun main() Int { 0 }
      ''');
      expect(
        harness.errorMessages.where((it) => it.startsWith('Cycle')),
        isNotEmpty,
      );
    });
  });

  group('main', () {
    test('resolves a valid main function', () {
      final harness = _lower('fun main() Int { 0 }');
      harness.expectNoErrors();
      expect('${harness.hir.mainFunction}', 'Core.test:main');
    });

    test('does not report a missing main function', () {
      final harness = _lower('fun other() Int { 0 }');
      harness.expectNoErrors();
      expect(harness.hir.mainFunction, isNull);
    });

    test('reports a main function with parameters', () {
      final harness = _lower('fun main(a: Int) Int { 0 }');
      expect(harness.errorMessages, [
        'The `main` function must not have parameters.',
      ]);
    });

    test('reports a main function returning the wrong type', () {
      final harness = _lower('fun main() Text { "x" }');
      expect(harness.errorMessages, [
        'The `main` function must return an `Int`.',
      ]);
    });
  });
}

LoweringHarness _lower(String source) => .full('$preludeSource\n$source');

/// Enough of a trait hierarchy to exercise dispatch and blanket impls.
const _traitsSource = '''
trait ToText { fun toText(self: Self) Text }
trait Compare { fun compareTo(self: Self, other: Self) Ordering }
trait Equal { fun equals(self: Self, other: Self) Bool }

impl Ordering: Compare {
  fun compareTo(self: Ordering, other: Ordering) Ordering { other }
}
impl Int: ToText { fun toText(self: Int) Text { builtinIntToText(self) } }
impl[T: Compare] T: Equal {
  fun equals(self: T, other: T) Bool { Bool.true_() }
}
''';
