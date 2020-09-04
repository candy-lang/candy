import 'package:freezed_annotation/freezed_annotation.dart';

import '../../../lexer/token.dart';
import '../../../syntactic_entity.dart';
import '../../../utils.dart';
import '../node.dart';
import '../statements.dart';

part 'expression.freezed.dart';

abstract class Expression extends Statement {
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
  const factory Identifier(IdentifierToken value) = _Identifier;
  const Identifier._();

  @override
  Iterable<SyntacticEntity> get children => [value];
}

@freezed
abstract class GroupExpression extends Expression implements _$GroupExpression {
  const factory GroupExpression({
    @required OperatorToken leftParenthesis,
    @required Expression expression,
    @required OperatorToken rightParenthesis,
  }) = _ParenthesizedExpression;
  const GroupExpression._();

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
abstract class NavigationExpression extends Expression
    implements _$NavigationExpression {
  const factory NavigationExpression({
    @required Expression target,
    @required OperatorToken dot,
    @required IdentifierToken name,
  }) = _NavigationExpression;
  const NavigationExpression._();

  @override
  Iterable<SyntacticEntity> get children => [target, dot, name];
}

@freezed
abstract class CallExpression extends Expression implements _$CallExpression {
  const factory CallExpression({
    @required Expression target,
    @required OperatorToken leftParenthesis,
    @Default(<Argument>[]) List<Argument> arguments,
    @Default(<OperatorToken>[]) List<OperatorToken> argumentCommata,
    @required OperatorToken rightParenthesis,
  }) = _CallExpression;
  const CallExpression._();

  @override
  Iterable<SyntacticEntity> get children => [
        target,
        if (leftParenthesis != null)
          leftParenthesis,
        ...interleave(arguments, argumentCommata),
        if (rightParenthesis != null)
          rightParenthesis,
      ];
}

@freezed
abstract class Argument extends AstNode implements _$Argument {
  const factory Argument({
    IdentifierToken name,
    OperatorToken equals,
    @required Expression expression,
  }) = _Argument;
  const Argument._();
  //     assert((name != null) == (equals != null)),

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

@freezed
abstract class IfExpression extends Expression implements _$IfExpression {
  const factory IfExpression({
    @required IfKeywordToken ifKeyword,
    @required Expression condition,
    @required Statement thenStatement,
    ElseKeywordToken elseKeyword,
    Statement elseStatement,
  }) = _IfExpression;
  const IfExpression._();

  @override
  Iterable<SyntacticEntity> get children => [
        ifKeyword,
        condition,
        thenStatement,
        if (elseKeyword != null) elseKeyword,
        if (elseStatement != null) elseStatement,
      ];
}
