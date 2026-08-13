// ignore_for_file: missing-test-assertion

import 'package:test/test.dart';

import '../hir/lowering/harness.dart';

void main() {
  test('`use Builtins` brings builtin functions into scope', () {
    final harness = LoweringHarness.full('''
      use Builtins
      struct Int = builtin
      struct Text = builtin
      fun main() Int {
        builtinPrint(builtinIntToText(42))
        0
      }
    ''');
    harness.expectNoErrors();
  });

  test('builtin functions are unavailable without importing Builtins', () {
    final harness = LoweringHarness.full('''
      struct Int = builtin
      struct Text = builtin
      fun main() Int {
        builtinPrint(builtinIntToText(42))
        0
      }
    ''');
    expect(harness.errorMessages, isNotEmpty);
  });

  test('Builtins needs no package on disk', () {
    final harness = LoweringHarness.full('''
      use Builtins
      struct Int = builtin
      fun main() Int { 0 }
    ''');
    expect(
      harness.errorMessages,
      isNot(contains('Unknown package: `Builtins`.')),
    );
  });
}
