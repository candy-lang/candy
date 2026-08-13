import 'package:compiler_v4_1/ast.dart';
import 'package:compiler_v4_1/lowering/declarations.dart';
import 'package:test/test.dart';

import '../hir/lowering/harness.dart';

void main() {
  test('records the declared visibility', () {
    final harness = LoweringHarness.of('''
      struct A {}
      fun hidden() A { A() }
      package fun shared() A { A() }
      public fun exported() A { A() }
    ''');
    harness.expectNoErrors();

    final byName = {
      for (final function in harness.declarations.ofKind<CollectedFunction>())
        function.id.name: function.visibility,
    };
    expect(byName, {
      'hidden': AstVisibility.private,
      'shared': AstVisibility.package,
      'exported': AstVisibility.public,
    });
  });
}
