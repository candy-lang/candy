// ignore_for_file: missing-test-assertion

import 'package:test/test.dart';

import '../hir/lowering/harness.dart';

void main() {
  test('allows calling a private method within its own module', () {
    final harness = LoweringHarness.fullModules({
      'main': '''
        struct Int = builtin
        struct S {}
        impl S { fun f(self: S) Int { 0 } }
        fun g(s: S) Int { s.f() }
        fun main() Int { 0 }
      ''',
    });
    harness.expectNoErrors();
  });

  test('rejects calling a private method from another module', () {
    final harness = LoweringHarness.fullModules({
      'a': '''
        package struct Int = builtin
        package struct S {}
        impl S { fun f(self: S) Int { 0 } }
      ''',
      'main': '''
        use package.a
        fun g(s: S) Int { s.f() }
        fun main() Int { 0 }
      ''',
    });
    expect(harness.errorMessages, contains('The function `f` is private.'));
  });

  test('allows calling a package method within the package', () {
    final harness = LoweringHarness.fullModules({
      'a': '''
        package struct Int = builtin
        package struct S {}
        impl S { package fun f(self: S) Int { 0 } }
      ''',
      'main': '''
        use package.a
        fun g(s: S) Int { s.f() }
        fun main() Int { 0 }
      ''',
    });
    harness.expectNoErrors();
  });

  test('rejects calling a package method from another package', () {
    final harness = LoweringHarness.fullPackages({
      'core': {
        's': '''
          public struct Int = builtin
          public struct S {}
          impl S { package fun f(self: S) Int { 0 } }
        ''',
      },
      'app': {
        'main': '''
          use core.s
          fun g(s: S) Int { s.f() }
          fun main() Int { 0 }
        ''',
      },
    });
    expect(
      harness.errorMessages,
      contains('The function `f` is not visible here.'),
    );
  });

  test('allows calling a public method from another package', () {
    final harness = LoweringHarness.fullPackages({
      'core': {
        's': '''
          public struct Int = builtin
          public struct S {}
          impl S { public fun f(self: S) Int { 0 } }
        ''',
      },
      'app': {
        'main': '''
          use core.s
          fun g(s: S) Int { s.f() }
          fun main() Int { 0 }
        ''',
      },
    });
    expect(
      harness.errorMessages.where((it) => it.contains('`f`')),
      isEmpty,
    );
  });

  test('allows calling a trait method from another module', () {
    final harness = LoweringHarness.fullModules({
      'a': '''
        package struct Int = builtin
        package struct S {}
        package trait T { fun f(self: Self) Int }
        impl S: T { fun f(self: S) Int { 0 } }
      ''',
      'main': '''
        use package.a
        fun g(s: S) Int { s.f() }
        fun main() Int { 0 }
      ''',
    });
    harness.expectNoErrors();
  });
}
