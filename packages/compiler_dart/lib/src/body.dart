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
            b.addExpression(_compile(context, expression));
          },
        );
      }
    });
  },
);

dart.Expression _compile(QueryContext context, Expression expression) {
  return expression.when(
    identifier: (id, identifier) => identifier.when(
      this_: (_) => dart.refer('this'),
      super_: (_) => dart.refer('super'),
      it: null,
      field: null,
      trait: null,
      class_: null,
      property: null,
      parameter: null,
      printFunction: (_) => dart.refer('print', dartCoreUrl),
    ),
    literal: (id, literal) => literal.when(
      boolean: dart.literalBool,
      integer: dart.literalNum,
    ),
    call: (id, target, valueArguments) => dart.InvokeExpression.newOf(
      _compile(context, target),
      valueArguments.map((a) => _compile(context, a)).toList(),
      {},
      [],
    ),
    functionCall: (id, target, arguments) {
      final functionId = (target.identifier as PropertyIdentifier).id;
      final parameters =
          getFunctionDeclarationHir(context, functionId).parameters;
      return dart.InvokeExpression.newOf(
        _compile(context, target),
        [
          for (final parameter in parameters)
            _compile(context, arguments[parameter.name]),
        ],
        {},
        [],
      );
    },
  );
}
