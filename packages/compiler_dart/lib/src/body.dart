import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:strings/strings.dart' as strings;

import 'declarations/module.dart';
import 'type.dart';

final compilePropertyInitializer = Query<DeclarationId, Option<dart.Code>>(
  'dart.compilePropertyInitializer',
  evaluateAlways: true,
  provider: (context, declarationId) {
    assert(declarationId.isProperty);
    final hir = getPropertyDeclarationHir(context, declarationId);
    if (hir.initializer == null) return None();

    return Some(_compileExpression(context, hir.initializer).code);
  },
);
final compileBody = Query<DeclarationId, Option<dart.Code>>(
  'dart.compileBody',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final body = getBody(context, declarationId);
    if (body.isNone) return None();
    final expressions = body.value;

    final compiled = expressions
        .map((expression) => _compileExpression(context, expression).statement);
    return Some(dart.Block((b) => b.statements.addAll(compiled)));
  },
);
final compileExpression = Query<Expression, dart.Expression>(
  'dart.compileExpression',
  evaluateAlways: true,
  provider: _compileExpression,
);

dart.Expression _compileExpression(
    QueryContext context, Expression expression) {
  return expression.when(
    identifier: (id, identifier) => identifier.when(
      this_: () => dart.refer('this'),
      super_: (_) => dart.refer('super'),
      it: null,
      field: null,
      module: (id) => ModuleExpression(context, id),
      trait: null,
      class_: null,
      parameter: null,
      property: (target, name, _) {
        final compiledTarget = _compileExpression(context, target);
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
      localProperty: null,
    ),
    literal: (id, literal) => _compileLiteralString(context, literal),
    call: (id, target, valueArguments) => dart.InvokeExpression.newOf(
      _compileExpression(context, target),
      valueArguments.map((a) => _compileExpression(context, a)).toList(),
      {},
      [],
    ),
    functionCall: null,
    return_: (id, _, expression) {
      // TODO(JonasWanke): non-local returns
      return _compileExpression(context, expression).returned;
    },
  );
}

dart.Expression _compileLiteralString(QueryContext context, Literal literal) {
  return literal.when(
    boolean: dart.literalBool,
    integer: dart.literalNum,
    string: (parts) {
      if (parts.isEmpty) return dart.literalString('');
      if (parts.length == 1 && parts.single is LiteralStringLiteralPart) {
        final part = parts.single as LiteralStringLiteralPart;
        return dart.literalString(strings.escape(part.value));
      }

      final block = dart.Block((b) {
        final content = StringBuffer();
        var nextLocalId = 0;

        for (final part in parts) {
          part.when(
            literal: (literal) => content.write(strings.escape(literal)),
            interpolated: (expression) {
              final name = 'local\$${nextLocalId++}';
              final type = compileType(context, expression.type);
              b.addExpression(
                compileExpression(context, expression).assignFinal(name, type),
              );

              content.write('\${$name}');
            },
          );
        }

        b.addExpression(dart.literalString(content.toString()).returned);
      });

      return dart.Method((b) => b..body = block).closure.call([], {}, []);
    },
    lambda: null,
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
