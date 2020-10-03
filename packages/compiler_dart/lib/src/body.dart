import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'constants.dart';

final compileBody = Query<DeclarationId, dart.Code>(
  'dart.compileBody',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final statements = getBody(context, declarationId);

    return dart.Block((b) {
      for (final statement in statements) {
        statement.when(
          expression: (_, expression) {
            b.addExpression(_compile(expression));
          },
        );
      }
    });
  },
);

dart.Expression _compile(Expression expression) {
  return expression.when(
    identifier: (id, identifier) => identifier.when(
      this_: () => dart.refer('this'),
      super_: () => dart.refer('super'),
      it: null,
      field: null,
      trait: null,
      class_: null,
      property: null,
      parameter: null,
      printFunction: () => dart.refer('print', dartCoreUrl),
    ),
    literal: (id, literal) => literal.when(
      boolean: dart.literalBool,
      integer: dart.literalNum,
    ),
    call: (id, target, valueArguments) => dart.InvokeExpression.newOf(
        _compile(target),
        valueArguments.map((a) => _compile(a.expression)).toList(), {}, []),
  );
}
