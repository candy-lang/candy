import 'package:parser/parser.dart' as ast;

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';
import 'declarations/declarations.dart';
import 'declarations/function.dart';

final getBody = Query<DeclarationId, List<hir.Statement>>(
  'getBody',
  provider: (context, declarationId) =>
      lowerBodyAstToHir(context, declarationId).first,
);
final getBodyAstToHirIds = Query<DeclarationId, BodyAstToHirIds>(
  'getBodyAstToHirIds',
  provider: (context, declarationId) =>
      lowerBodyAstToHir(context, declarationId).second,
);
final lowerBodyAstToHir =
    Query<DeclarationId, Tuple2<List<hir.Statement>, BodyAstToHirIds>>(
  'lowerBodyAstToHir',
  provider: (context, declarationId) {
    if (declarationId.isFunction) {
      final functionAst = getFunctionDeclarationAst(context, declarationId);

      var nextValue = 0;
      var idMap = BodyAstToHirIds();
      DeclarationLocalId idProvider(int astId) {
        final id = DeclarationLocalId(declarationId, nextValue);
        idMap = idMap.withMapping(astId, id);
        nextValue++;
        return id;
      }

      final identifiers = <String, hir.Identifier>{
        for (final parameter in functionAst.valueParameters)
          parameter.name.name: hir.Identifier.parameter(parameter.name.name, 0),
      };
      final statements =
          functionAst.body.statements.map<hir.Statement>((statement) {
        if (statement is ast.Expression) {
          return hir.Statement.expression(
            idProvider(statement.id),
            _mapExpression(
              idProvider,
              statement,
              identifiers,
              declarationId.resourceId,
            ),
          );
        } else {
          throw CompilerError.unsupportedFeature(
            'Unsupported statement.',
            location: ErrorLocation(declarationId.resourceId, statement.span),
          );
        }
      }).toList();
      return Tuple2(statements, idMap);
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported body.',
        location: ErrorLocation(
          declarationId.resourceId,
          getDeclarationAst(context, declarationId).span,
        ),
      );
    }
  },
);

typedef IdProvider = DeclarationLocalId Function(int astId);

hir.Expression _mapExpression(
  IdProvider idProvider,
  ast.Expression expression,
  Map<String, hir.Identifier> identifiers,
  ResourceId resourceId,
) {
  hir.Expression map(ast.Expression expression) =>
      _mapExpression(idProvider, expression, identifiers, resourceId);

  if (expression is ast.Literal) {
    return hir.Expression.literal(
      idProvider(expression.id),
      _mapLiteral(expression.value, resourceId),
    );
  } else if (expression is ast.Identifier) {
    final identifier = expression.value.name;
    final known = identifiers[identifier];
    if (known != null) {
      return hir.Expression.identifier(idProvider(expression.id), known);
    }

    if (identifier == 'print') {
      return hir.Expression.identifier(
        idProvider(expression.id),
        hir.Identifier.printFunction(),
      );
    }
    throw CompilerError.undefinedIdentifier(
      "Couldn't resolve identifier `$identifier`",
      location: ErrorLocation(resourceId, expression.value.span),
    );
  } else if (expression is ast.CallExpression) {
    return hir.Expression.call(
      idProvider(expression.id),
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
      'Unsupported expression.',
      location: ErrorLocation(resourceId, expression.span),
    );
  }
}

hir.Literal _mapLiteral(
    ast.LiteralToken<dynamic> token, ResourceId resourceId) {
  if (token is ast.BooleanLiteralToken) return hir.Literal.boolean(token.value);
  if (token is ast.IntegerLiteralToken) return hir.Literal.integer(token.value);
  throw CompilerError.unsupportedFeature(
    'Unsupported literal.',
    location: ErrorLocation(resourceId, token.span),
  );
}
