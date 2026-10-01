// ignore_for_file: missing-test-assertion

import 'package:compiler_v4_1/hir/hir/hir.dart';
import 'package:test/test.dart';

import 'harness.dart';

void main() {
  group('trait object types', () {
    test('lowers a trait in type position to a trait object', () {
      final harness = _lower('''
        fun show(widget: Widget) Text { widget.describe() }
      ''');
      harness.expectNoErrors();
      expect(
        harness.functionNamed('show').signature.parameters.single.type,
        isA<HirType_Dyn>(),
      );
      expect(harness.callsIn('show'), [contains('Widget.describe')]);
    });

    test('reports each kind of function that has no vtable slot', () {
      final harness = _lower('''
        trait Generic { fun convert[T](self: Self) T }
        trait Binary { fun combine(self: Self, other: Self) Self }
        trait Nested { fun wrap(self: Self) Maybe[Self] }
        trait Stateful {
          type State
          fun state(self: Self) Self.State
        }
        trait Static { fun create() Int }
        fun a(value: Generic) Int { 0 }
        fun b(value: Binary) Int { 0 }
        fun c(value: Nested) Int { 0 }
        fun d(value: Stateful) Int { 0 }
        fun e(value: Static) Int { 0 }
      ''');
      expect(harness.errorMessages, [
        // ignore: no_adjacent_strings_in_list
        "`Generic` can't be used as a type: its function `convert` has type "
            'parameters of its own.',
        // ignore: no_adjacent_strings_in_list
        "`Binary` can't be used as a type: its function `combine` takes "
            'another `Self` besides the receiver.',
        // ignore: no_adjacent_strings_in_list
        "`Nested` can't be used as a type: its function `wrap` returns `Self` "
            'nested inside another type.',
        // ignore: no_adjacent_strings_in_list
        "`Stateful` can't be used as a type: its function `state` uses an "
            'associated type of `Self`.',
        // ignore: no_adjacent_strings_in_list
        "`Static` can't be used as a type: its function `create` doesn't take "
            '`self: Self` as its first parameter.',
      ]);
    });

    test('accepts associated types that no function projects', () {
      final harness = _lower('''
        trait Stateful {
          type State
          fun name(self: Self) Text
        }
        fun f(value: Stateful) Text { value.name() }
      ''');
      harness.expectNoErrors();
    });

    test('accepts a bare `Self` return', () {
      final harness = _lower('''
        trait Copy { fun copy(self: Self) Self }
        struct Point { x: Int }
        impl Point: Copy { fun copy(self: Point) Point { self } }
        fun f(value: Copy) Copy { value.copy() }
      ''');
      harness.expectNoErrors();
    });

    test(
      'accepts a defaulted function without a slot, but not calling it on an '
      'object',
      () {
        final harness = _lower('''
          trait Named {
            fun name(self: Self) Text
            fun same(self: Self, other: Self) Text { other.name() }
          }
          fun f(a: Named, b: Named) Text { a.same(b) }
        ''');
        expect(harness.errorMessages, [startsWith('No function `same`')]);
      },
    );

    test('reports an impl for a trait object', () {
      final harness = _lower('''
        impl Widget { fun f(self: Widget) Int { 0 } }
        fun main() Int { 0 }
      ''');
      expect(harness.errorMessages, ["Can't implement for a trait object."]);
    });

    test('reports an impl of `Any`', () {
      final harness = _lower('''
        impl Label: Any {}
        fun main() Int { 0 }
      ''');
      expect(harness.errorMessages, ['Every type implements `Any` already.']);
    });
  });

  group('coercion', () {
    test('boxes a `let` with an annotation', () {
      final harness = _lower('''
        fun f() Text {
          let widget: Widget = Label("a")
          widget.describe()
        }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
    });

    test('boxes a return value', () {
      final harness = _lower('fun f() Widget { Label("a") }');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
    });

    test('boxes a struct field', () {
      final harness = _lower('''
        struct Padding { child: Widget }
        fun f() Padding { Padding(Label("a")) }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
    });

    test("boxes an enum payload, inferring the enum's type argument", () {
      final harness = _lower('''
        fun f() Maybe[Widget] { Maybe.some(Label("a")) }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
    });

    test('boxes an argument of the only candidate', () {
      final harness = _lower('''
        fun show(widget: Widget) Text { widget.describe() }
        fun f() Text { show(Counter(1)) }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
    });

    test('boxes an argument once overloads are resolved', () {
      final harness = _lower('''
        fun show(widget: Widget, count: Int) Text { widget.describe() }
        fun show(widget: Widget, text: Text) Text { text }
        fun f() Text { show(Counter(1), 2) }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
      expect(harness.bodyTextOf('f'), contains('toDyn'));
    });

    test('builds a heterogeneous list from the expected type', () {
      final harness = _lower('''
        fun f() Column { Column(List.of(Label("hi"), Counter(1))) }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 2);
    });

    test('suggests a trait when list items disagree', () {
      final harness = _lower('''
        fun f() Int {
          let items = List.of(Label("hi"), Counter(1))
          0
        }
      ''');
      expect(harness.errorMessages.join('\n'), contains('to mix types'));
    });

    test('boxes a type parameter bounded by the trait', () {
      final harness = _lower('''
        fun box[T: Widget](value: T) Widget { value }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('box'), 1);
    });

    test('lets a trait object satisfy a bound on its trait', () {
      final harness = _lower('''
        fun describeTwice[T: Widget](value: T) Text { value.describe() }
        fun f() Text {
          let widget: Widget = Label("a")
          describeTwice(widget)
        }
      ''');
      harness.expectNoErrors();
    });

    test('upcasts a trait object to `Any`', () {
      final harness = _lower('''
        fun f(widget: Widget) Any { widget }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
    });

    test("doesn't box into an unrelated trait", () {
      final harness = _lower('''
        trait Other { fun other(self: Self) Int }
        fun f() Other { Label("a") }
      ''');
      expect(harness.errorMessages, ['Expected type `Other`, got `Label`.']);
    });

    test('prefers a concrete overload over a trait object one', () {
      final harness = _lower('''
        fun pick(label: Label) Int { 1 }
        fun pick(widget: Widget) Int { 2 }
        fun f() Int { pick(Label("a")) }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 0);
    });
  });

  group('Any', () {
    test('is implemented by every type, trait objects included', () {
      final harness = _lower('''
        fun f[T](value: T, widget: Widget) Int {
          let a: TypeId = Label("a").typeId()
          let b: TypeId = value.typeId()
          let c: TypeId = widget.typeId()
          let d: Bool = widget.is[Label]()
          let e: Maybe[Label] = widget.downcast[Label]()
          0
        }
      ''');
      harness.expectNoErrors();
    });

    test('accepts any value', () {
      final harness = _lower('''
        fun f() Any { 1 }
      ''');
      harness.expectNoErrors();
      expect(harness.coercionsIn('f'), 1);
    });
  });
}

LoweringHarness _lower(String source) => LoweringHarness.full('''
  $preludeSource
  $anyPreludeSource
  impl[T] List[T] {
    fun of(item0: T, item1: T) List[T] { builtinListOf(item0, item1) }
  }
  trait Widget { fun describe(self: Self) Text }
  struct Label { text: Text }
  impl Label: Widget { fun describe(self: Label) Text { self.text } }
  struct Counter { count: Int }
  impl Counter: Widget {
    fun describe(self: Counter) Text { builtinIntToText(self.count) }
  }
  struct Column { children: List[Widget] }
  $source
''');

extension on LoweringHarness {
  /// How many values [name]'s body boxes into trait objects.
  int coercionsIn(String name) {
    var count = 0;
    void visit(HirBody body) {
      for (final entry in body.expressions) {
        switch (entry.expression.kind) {
          case HirExpressionKind_ToDyn():
            count++;
          case HirExpressionKind_Switch(:final cases):
            for (final case_ in cases) {
              visit(case_.body);
            }
          case HirExpressionKind_Lambda(:final body):
            visit(body);
          default:
            break;
        }
      }
    }

    visit((functionNamed(name).body as HirBodyOrBuiltin_Body).body);
    return count;
  }
}
