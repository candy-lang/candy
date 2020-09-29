import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

import '../../errors.dart';
import '../ast/parser.dart';
import '../../query.dart';
import '../../utils.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';
import 'declarations/declarations.dart';
import 'declarations/module.dart';
import 'declarations/function.dart';
import 'declarations/trait.dart';

final getBody = Query<DeclarationId, List<hir.Statement>>(
  'getBody',
  provider: (context, declarationId) {
    if (declarationId.isFunction) {
      final ast = getFunctionDeclarationAst(context, declarationId);

      final identifiers = <String, hir.Identifier>{
        for (final parameter in ast.valueParameters)
          parameter.name.name: hir.Identifier.parameter(parameter.name.name, 0),
      };
      ast.body.statements.map((statement) {
        if (statement is ast.Expression) {
        } else {
          throw CompilerError.internalError(
            'Unknown statement',
            location: ErrorLocation(declarationId.resourceId, statement.span),
          );
        }
      });
    }
  },
);

hir.Expression _mapExpression(
  ast.Expression expression,
  Map<String, hir.Identifier> identifiers,
  ResourceId resourceId,
) {
  hir.Expression map(ast.Expression expression) =>
      _mapExpression(expression, identifiers, resourceId);

  if (expression is ast.Literal) {
    return hir.Expression.literal(_mapLiteral(expression.value, resourceId));
  } else if (expression is ast.Identifier) {
    final identifier = expression.value.name;
    final known = identifiers[identifier];
    if (known != null) return hir.Expression.identifier(known);

    if (identifier == 'print') {
      return hir.Expression.identifier(hir.Identifier.printFunction());
    }
    throw CompilerError.undefinedIdentifier(
      "Couldn't resolve identifier `$identifier`",
      location: ErrorLocation(resourceId, expression.value.span),
    );
  } else if (expression is ast.CallExpression) {
    return hir.Expression.call(
      map(expression.target),
      expression.arguments
          .map((argument) => hir.ValueArgument(
                name: argument.name?.name,
                expression: map(argument.expression),
              ))
          .toList(),
    );
  } else {
    throw CompilerError.unsupportedFeature(
      'Unknown expression',
      location: ErrorLocation(resourceId, expression.span),
    );
  }
}

hir.Literal _mapLiteral(
    ast.LiteralToken<dynamic> token, ResourceId resourceId) {
  if (token is ast.BooleanLiteralToken) return hir.Literal.boolean(token.value);
  if (token is ast.IntegerLiteralToken) return hir.Literal.integer(token.value);
  throw CompilerError.internalError(
    'Unknown literal',
    location: ErrorLocation(resourceId, token.span),
  );
}
