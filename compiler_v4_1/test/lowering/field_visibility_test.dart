// ignore_for_file: missing-test-assertion

import 'package:test/test.dart';

import '../hir/lowering/harness.dart';

void main() {
  test('allows access to a private field within its own module', () {
    final harness = LoweringHarness.fullModules({
      'main': '''
        struct Int = builtin
        struct S { a: Int }
        fun f(s: S) Int { s.a }
        fun main() Int { 0 }
      ''',
    });
    harness.expectNoErrors();
  });

  test('rejects access to a private field from another module', () {
    final harness = LoweringHarness.fullModules({
      'a': 'package struct Int = builtin\npackage struct S { a: Int }',
      'main': '''
        use package.a
        fun f(s: S) Int { s.a }
        fun main() Int { 0 }
      ''',
    });
    expect(harness.errorMessages, contains('The field `a` is private.'));
  });

  test('allows access to a package field within the package', () {
    final harness = LoweringHarness.fullModules({
      'a': '''
        package struct Int = builtin
        package struct S { package a: Int }
      ''',
      'main': '''
        use package.a
        fun f(s: S) Int { s.a }
        fun main() Int { 0 }
      ''',
    });
    harness.expectNoErrors();
  });

  test('rejects access to a package field from another package', () {
    final harness = LoweringHarness.fullPackages({
      'core': {
        's': '''
          public struct Int = builtin
          public struct S { package a: Int }
        ''',
      },
      'app': {
        'main': '''
          use core.s
          fun f(s: S) Int { s.a }
          fun main() Int { 0 }
        ''',
      },
    });
    expect(
      harness.errorMessages,
      contains('The field `a` is not visible here.'),
    );
  });

  test('rejects construction when a field is not visible', () {
    final harness = LoweringHarness.fullModules({
      'a': 'package struct Int = builtin\npackage struct S { a: Int }',
      'main': '''
        use package.a
        fun f() S { S(1) }
        fun main() Int { 0 }
      ''',
    });
    expect(
      harness.errorMessages,
      contains('Cannot construct `S` here: the field `a` is private.'),
    );
  });
}
