// ignore_for_file: missing-test-assertion

import 'package:compiler_v4_1/mono/mono/ids.dart';
import 'package:compiler_v4_1/mono/mono/mono.dart';
import 'package:test/test.dart';

import '../hir/lowering/harness.dart';
import 'harness.dart';

void main() {
  group('vtables', () {
    test('resolves direct, blanket, and inherited impls', () {
      final harness = _monomorphize('''
        trait Named { fun name(self: Self) Text }
        struct Button {}
        impl Button: Named { fun name(self: Button) Text { "button" } }
        impl[T: Named] T: Widget {
          fun describe(self: T) Text { self.name() }
        }
        fun main() Int {
          let label: Widget = Label("a")
          let button: Widget = Button()
          0
        }
      ''');
      expect(harness.vtableEntries('Label', 'Widget'), [
        r'$impl#1.describe',
        'Widget.title[Label]',
      ]);
      expect(harness.vtableEntries('Button', 'Widget'), [
        r'$impl#3.describe[Button]',
        'Widget.title[Button]',
      ]);
    });

    test('gives each type its own ID, shared by every use', () {
      final harness = _monomorphize('''
        fun main() Int {
          let a: Widget = Label("a")
          let b: Widget = Label("b")
          let c: Any = Label("c")
          let d: Any = 1
          let e: TypeId = TypeId.of[Label]()
          0
        }
      ''');
      final labelIds = harness.mono.vtables.entries
          .where((it) => describeType(it.key.concreteType) == 'Label')
          .map((it) => it.value.typeId)
          .toSet();
      expect(labelIds, hasLength(1));
      expect(harness.mono.typeIds.values.toSet(), hasLength(2));
    });
  });

  group('dispatch', () {
    test('calls through the vtable slot of the trait function', () {
      final harness = _monomorphize('''
        fun main() Int {
          let widget: Widget = Label("a")
          widget.title()
          widget.describe()
          0
        }
      ''');
      expect(
        harness.instance('Widget.describe[Widget]').body,
        const MonoBodyOrBuiltin.dynDispatch(slot: 0),
      );
      expect(
        harness.instance('Widget.title[Widget]').body,
        const MonoBodyOrBuiltin.dynDispatch(slot: 1),
      );
    });

    test('gives a `Self`-returning slot the object type', () {
      final harness = _monomorphize('''
        trait Copy { fun copy(self: Self) Self }
        impl Label: Copy { fun copy(self: Label) Label { self } }
        fun main() Int {
          let value: Copy = Label("a")
          value.copy()
          0
        }
      ''');
      final declaration = harness.mono.typeDeclarations.entries
          .singleWhere((it) => describeType(it.key) == 'Copy')
          .value;
      expect(
        (declaration as MonoTypeDeclaration_Dyn).slots.single.returnType,
        isA<MonoType_Dyn>(),
      );
      expect(harness.vtableEntries('Label', 'Copy'), [r'$impl#2.copy']);
    });

    test("runs `Any`'s defaults on a trait object", () {
      final harness = _monomorphize('''
        fun main() Int {
          let widget: Widget = Label("a")
          widget.downcast[Label]()
          0
        }
      ''');
      expect(harness.callsIn('Any.downcast[Widget, Label]'), [
        'builtinDowncast[Label, Widget]',
      ]);
    });
  });

  group('boxing', () {
    test('rereads a trait object as an `Any` without a vtable', () {
      final harness = _monomorphize('''
        fun upcast(widget: Widget) Any { widget }
        fun main() Int {
          upcast(Label("a"))
          0
        }
      ''');
      expect(harness.boxingsIn('upcast'), [null]);
    });

    test('drops a coercion that substitution makes trivial', () {
      final harness = _monomorphize('''
        fun box[T: Widget](value: T) Widget { value }
        fun main() Int {
          let widget: Widget = Label("a")
          box(widget)
          box(Label("b"))
          0
        }
      ''');
      expect(harness.boxingsIn('box[Widget]'), isEmpty);
      expect(harness.boxingsIn('box[Label]'), ['Label as Widget']);
    });
  });
}

MonoHarness _monomorphize(String source) => MonoHarness.of('''
  $anyPreludeSource
  trait Widget {
    fun describe(self: Self) Text
    fun title(self: Self) Text { "untitled" }
  }
  struct Label { text: Text }
  impl Label: Widget { fun describe(self: Label) Text { self.text } }
  $source
''');

extension on MonoHarness {
  /// The functions in the vtable for [concrete] as [trait], in slot order.
  List<String> vtableEntries(String concrete, String trait) => mono
      .vtables
      .entries
      .singleWhere(
        (it) =>
            describeType(it.key.concreteType) == concrete &&
            describeType(it.key.dynType) == trait,
      )
      .value
      .entries
      .map(describeInstance)
      .toList();

  /// Each value [description] boxes, as `Label as Widget`, or `null` for an
  /// upcast that reuses the existing vtable.
  List<String?> boxingsIn(String description) => [
    for (final entry
        in (instance(description).body as MonoBodyOrBuiltin_Body)
            .body
            .expressions)
      if (entry.expression.kind case MonoExpressionKind_ToDyn(:final vtable))
        vtable == null
            ? null
            : '${describeType(vtable.concreteType)} as '
                  '${describeType(vtable.dynType)}',
  ];
}
