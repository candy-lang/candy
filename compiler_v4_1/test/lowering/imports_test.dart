// ignore_for_file: missing-test-assertion

import 'package:test/test.dart';

import '../hir/lowering/harness.dart';

/// `astToHir` insists on a `main` returning `Int`, so the entry module of every
/// full-pipeline harness below carries both.
const _entry = 'struct Int = builtin\nfun main() Int { 0 }';

void main() {
  test('resolves a name from an imported module', () {
    final harness = LoweringHarness.fullModules({
      'a': 'package struct A {}\npackage fun fromA() A { A() }',
      'main': 'use package.a\n$_entry\nfun f() A { fromA() }',
    });
    harness.expectNoErrors();
  });

  test('does not import a private declaration', () {
    final harness = LoweringHarness.fullModules({
      'a': 'public struct A {}\nfun hidden() A { A() }',
      'main': 'use package.a\n$_entry\nfun f() A { hidden() }',
    });
    expect(harness.errorMessages, isNotEmpty);
  });

  test('does not import transitively', () {
    final direct = LoweringHarness.fullModules({
      'a': 'package struct A {}\npackage fun fromA() A { A() }',
      'b': 'use package.a',
      'main': 'use package.b\nuse package.a\n$_entry\nfun f() A { fromA() }',
    });
    direct.expectNoErrors();

    final indirect = LoweringHarness.fullModules({
      'a': 'package struct A {}\npackage fun fromA() A { A() }',
      'b': 'use package.a',
      'main': 'use package.b\n$_entry\nfun f() A { fromA() }',
    });
    expect(indirect.errorMessages, isNotEmpty);
  });

  test('merges overloads from two imported modules', () {
    final harness = LoweringHarness.fullModules({
      'shared': 'package struct A {}\npackage struct B {}',
      'a': 'use package.shared\npackage fun parse(a: A) A { a }',
      'b': 'use package.shared\npackage fun parse(a: B) B { a }',
      'main':
          '''
        use package.shared
        use package.a
        use package.b
        $_entry
        fun f(a: A) A { parse(a) }
        fun g(b: B) B { parse(b) }
      ''',
    });
    harness.expectNoErrors();
  });

  test('does not shadow a local declaration', () {
    final harness = LoweringHarness.fullModules({
      'a': 'package struct A {}\npackage fun make() A { A() }',
      'main':
          '''
        use package.a
        $_entry
        struct Local {}
        fun make() Local { Local() }
        fun f() Local { make() }
      ''',
    });
    harness.expectNoErrors();
  });

  test('allows a cycle between two modules', () {
    final harness = LoweringHarness.fullModules({
      'a': '''
        use package.b
        package struct A {}
        package fun fromA() A { fromB() }
      ''',
      'b': 'use package.a\npackage fun fromB() A { A() }',
      'main': 'use package.a\n$_entry\nfun f() A { fromA() }',
    });
    harness.expectNoErrors();
  });

  test('reports an ambiguous type only where it is named', () {
    final harness = LoweringHarness.fullModules({
      'a': 'package struct Buffer {}\npackage fun fromA() Buffer { Buffer() }',
      'b': 'package struct Buffer {}',
      'main':
          'use package.a\nuse package.b\n$_entry\nfun f(x: Buffer) Int { 0 }',
    });
    expect(harness.errorMessages, [
      'Ambiguous name `Buffer`, imported from `Core.a` and `Core.b`.',
    ]);
  });

  test('leaves unambiguous names from the same modules usable', () {
    final harness = LoweringHarness.fullModules({
      'a': 'package struct Buffer {}\npackage fun fromA() Buffer { Buffer() }',
      'b': 'package struct Buffer {}',
      'main': 'use package.a\nuse package.b\n$_entry\nfun f() Int { 0 }',
    });
    harness.expectNoErrors();
  });

  test('reports an unknown module', () {
    final harness = LoweringHarness.modules({'a': 'use package.nope'});
    expect(harness.errorMessages, ['Unknown module: `package.nope`.']);
  });

  test('reports a self-import', () {
    final harness = LoweringHarness.modules({'a': 'use package.a'});
    expect(harness.errorMessages, ["A module can't import itself."]);
  });

  test('reports a duplicate import', () {
    final harness = LoweringHarness.modules({
      'a': 'package struct A {}',
      'b': 'use package.a\nuse package.a',
    });
    expect(harness.errorMessages, ['`package.a` is already imported.']);
  });

  test('reports an unknown package', () {
    final harness = LoweringHarness.modules({'a': 'use other.thing'});
    expect(harness.errorMessages, ['Unknown package: `other`.']);
  });

  test('imports a public declaration from another package', () {
    final harness = LoweringHarness.packages({
      'core': {
        'text': 'public struct Text {}\npublic fun trim(a: Text) Text { a }',
      },
      'app': {'main': 'use core.text\nfun f(a: Text) Text { trim(a) }'},
    });
    harness.expectNoErrors();
  });

  test('does not import a package-visible function across packages', () {
    final harness = LoweringHarness.fullPackages({
      'core': {
        'text': 'public struct Text {}\npackage fun trim(a: Text) Text { a }',
      },
      'app': {
        'main': 'use core.text\n$_entry\nfun f(a: Text) Text { trim(a) }',
      },
    });
    expect(harness.errorMessages, isNotEmpty);
  });

  test('does not import a package-visible type across packages', () {
    final harness = LoweringHarness.packages({
      'core': {'text': 'package struct Text {}'},
      'app': {'main': 'use core.text\nfun f(a: Text) Text { a }'},
    });
    expect(harness.errorMessages, [
      'Unknown type: `Text`.',
      'Unknown type: `Text`.',
    ]);
  });

  test('merges an overload reachable through both the prelude and a use', () {
    final harness = LoweringHarness.fullPackages({
      'Core': {
        'text': '''
          public struct Int = builtin
          public struct Text {}
          public fun trim(a: Text) Text { a }
        ''',
      },
      'app': {
        'main': '''
          use Core.text
          fun main() Int { 0 }
          fun f(a: Text) Text { trim(a) }
        ''',
      },
    });
    harness.expectNoErrors();
  });

  test('allows a cycle between two packages', () {
    final harness = LoweringHarness.packages({
      'a': {
        'main': '''
          use b.main
          public struct A {}
          public fun fromA() A { fromB() }
        ''',
      },
      'b': {'main': 'use a.main\npublic fun fromB() A { A() }'},
    });
    harness.expectNoErrors();
  });
}
