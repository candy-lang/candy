import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:parser/src/lexer/token.dart';

import '../../../syntactic_entity.dart';
import '../node.dart';

part 'expression.freezed.dart';

abstract class Expression extends AstNode {
  const Expression();
}

@freezed
abstract class Literal<T> extends Expression implements _$Literal<T> {
  const factory Literal(LiteralToken<T> value) = _Literal<T>;
  const Literal._();

  @override
  Iterable<SyntacticEntity> get children => [value];
}

@freezed
abstract class Identifier extends Expression implements _$Identifier {
  const factory Identifier(SimpleIdentifierToken value) = _Identifier;
  const Identifier._();

  @override
  Iterable<SyntacticEntity> get children => [value];
}

@freezed
abstract class ParenthesizedExpression extends Expression
    implements _$ParenthesizedExpression {
  const factory ParenthesizedExpression({
    @required OperatorToken leftParenthesis,
    @required Expression expression,
    @required OperatorToken rightParenthesis,
  }) = _ParenthesizedExpression;
  const ParenthesizedExpression._();

  // assert(leftParenthesis.type == OperatorTokenType.lparen),

  @override
  Iterable<SyntacticEntity> get children => [
        leftParenthesis,
        expression,
        rightParenthesis,
      ];
}

abstract class OperatorExpression extends Expression {
  const OperatorExpression();

  OperatorToken get operatorToken;

  // TODO(JonasWanke): actual operator
  // Operator get operator => operatorToken.type;
}

abstract class UnaryExpression extends OperatorExpression {
  const UnaryExpression();

  Expression get operand;
}

@freezed
abstract class PrefixExpression extends UnaryExpression
    implements _$PrefixExpression {
  const factory PrefixExpression({
    @required OperatorToken operatorToken,
    @required Expression operand,
  }) = _PrefixExpression;
  const PrefixExpression._();

  @override
  Iterable<SyntacticEntity> get children => [operatorToken, operand];
}

@freezed
abstract class PostfixExpression extends UnaryExpression
    implements _$PostfixExpression {
  const factory PostfixExpression({
    @required Expression operand,
    @required OperatorToken operatorToken,
  }) = _PostfixExpression;
  const PostfixExpression._();

  @override
  Iterable<SyntacticEntity> get children => [operand, operatorToken];
}

@freezed
abstract class BinaryExpression extends OperatorExpression
    implements _$BinaryExpression {
  const factory BinaryExpression(
    Expression leftOperand,
    OperatorToken operatorToken,
    Expression rightOperand,
  ) = _BinaryExpression;
  const BinaryExpression._();

  @override
  Iterable<SyntacticEntity> get children =>
      [leftOperand, operatorToken, rightOperand];
}

@freezed
abstract class InvocationExpression extends Expression
    implements _$InvocationExpression {
  const factory InvocationExpression({
    @required Expression target,
    @required OperatorToken leftParenthesis,
    @required List<Argument> arguments,
    @required OperatorToken rightParenthesis,
  }) = _InvocationExpression;
  const InvocationExpression._();

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

@freezed
abstract class Argument extends AstNode implements _$Argument {
  const factory Argument({
    SimpleIdentifierToken name,
    OperatorToken equals,
    @required Expression expression,
  }) = _Argument;
  const Argument._();

  @override
  Iterable<SyntacticEntity> get children => [
        if (name != null) name,
        if (equals != null) equals,
        expression,
      ];
}

@freezed
abstract class IndexExpression extends Expression implements _$IndexExpression {
  const factory IndexExpression({
    @required Expression target,
    @required OperatorToken leftSquareBracket,
    @required List<Expression> indices,
    @required OperatorToken rightSquareBracket,
  }) = _IndexExpression;
  const IndexExpression._();

  @override
  Iterable<SyntacticEntity> get children =>
      [target, leftSquareBracket, ...indices, rightSquareBracket];
}
