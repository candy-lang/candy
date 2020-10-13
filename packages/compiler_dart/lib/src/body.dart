import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'constants.dart';
import 'declarations/module.dart';
import 'type.dart';

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
      module: (id) => ModuleExpression(context, id),
      trait: null,
      class_: null,
      property: (target, name, _) {
        final compiledTarget = _compile(context, target);
        if (compiledTarget is ModuleExpression) {
          final currentModuleId =
              declarationIdToModuleId(context, expression.id.declarationId);
          if (compiledTarget.moduleId == currentModuleId) {
            return dart.refer(name);
          }

          return dart.refer(
            name,
            moduleIdToImportUrl(context, compiledTarget.moduleId),
          );
        }
        return compiledTarget.property(name);
      },
      parameter: null,
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
      final functionId = getPropertyIdentifierDeclarationId(
        context,
        target.identifier as PropertyIdentifier,
      );
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

class ModuleExpression extends dart.InvokeExpression {
  ModuleExpression(QueryContext context, this.moduleId)
      : assert(context != null),
        assert(moduleId != null),
        super.constOf(
          compileType(context, CandyType.moduleDeclaration),
          [dart.literalString(moduleId.toString())],
          {},
          [],
        );

  final ModuleId moduleId;
}
