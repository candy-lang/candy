import 'package:parser/src/lexer/token.dart';

import '../../../syntactic_entity.dart';
import '../node.dart';

abstract class Expression extends AstNode {
  const Expression();
}

class ParenthesizedExpression extends Expression {
  const ParenthesizedExpression(
    this.leftParenthesis,
    this.expression,
    this.rightParenthesis,
  )   : assert(leftParenthesis != null),
        assert(expression != null),
        assert(rightParenthesis != null);

  final OperatorToken leftParenthesis;
  final Expression expression;
  final OperatorToken rightParenthesis;

  @override
  Iterable<AstNode> get children => [expression];
}

abstract class OperatorExpression extends Expression {
  const OperatorExpression(this.operatorToken) : assert(operatorToken != null);

  final OperatorToken operatorToken;

  // TODO(JonasWanke): actual operator
  // Operator get operator => operatorToken.type;
}

abstract class UnaryExpression extends OperatorExpression {
  const UnaryExpression(
    OperatorToken operatorToken,
    this.operand,
  )   : assert(operand != null),
        super(operatorToken);

  final Expression operand;
}

class PrefixExpression extends UnaryExpression {
  const PrefixExpression(
    OperatorToken operatorToken,
    Expression operand,
  )   : assert(operand != null),
        super(operatorToken, operand);

  @override
  Iterable<SyntacticEntity> get children => [operatorToken, operand];
}

class PostfixExpression extends UnaryExpression {
  const PostfixExpression(
    Expression operand,
    OperatorToken operatorToken,
  )   : assert(operand != null),
        super(operatorToken, operand);

  @override
  Iterable<SyntacticEntity> get children => [operand, operatorToken];
}

class BinaryExpression extends OperatorExpression {
  const BinaryExpression(
    this.leftOperand,
    OperatorToken operatorToken,
    this.rightOperand,
  )   : assert(leftOperand != null),
        assert(rightOperand != null),
        super(operatorToken);

  final Expression leftOperand;
  final Expression rightOperand;

  @override
  Iterable<SyntacticEntity> get children =>
      [leftOperand, operatorToken, rightOperand];
}

class InvocationExpression extends Expression {
  const InvocationExpression(
    this.target,
    this.leftParenthesis,
    this.arguments,
    this.rightParenthesis,
  )   : assert(target != null),
        assert(leftParenthesis != null),
        assert(arguments != null),
        assert(rightParenthesis != null);

  final Expression target;
  final OperatorToken leftParenthesis;
  final List<Argument> arguments;
  final OperatorToken rightParenthesis;

  @override
  Iterable<SyntacticEntity> get children => [
        target,
        if (leftParenthesis != null)
          leftParenthesis,
        ...arguments,
        if (rightParenthesis != null)
          rightParenthesis,
      ];
}

class Argument extends AstNode {
  const Argument(this.name, this.equals, this.expression)
      : assert((name != null) == (equals != null)),
        assert(expression != null);

  final SimpleIdentifier name;
  final OperatorToken equals;
  final Expression expression;

  @override
  Iterable<SyntacticEntity> get children => [
        if (name != null) name,
        if (equals != null) equals,
        expression,
      ];
}

class IndexExpression extends Expression {
  const IndexExpression(
    this.target,
    this.leftSquareBracket,
    this.indices,
    this.rightSquareBracket,
  )   : assert(target != null),
        assert(leftSquareBracket != null),
        assert(indices != null),
        assert(rightSquareBracket != null);

  final Expression target;
  final OperatorToken leftSquareBracket;
  final List<Expression> indices;
  final OperatorToken rightSquareBracket;

  @override
  Iterable<SyntacticEntity> get children =>
      [target, leftSquareBracket, ...indices, rightSquareBracket];
}
