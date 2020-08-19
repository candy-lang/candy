import 'package:meta/meta.dart';
import 'package:petitparser/petitparser.dart' hide ChoiceParserExtension;

import '../lexer/lexer.dart';
import '../syntactic_entity.dart';
import '../utils.dart';
import 'ast/expressions/expression.dart';
import 'ast/expressions/literal.dart';

// ignore: avoid_classes_with_only_static_members
@immutable
class ParserGrammar {
  static void init() {
    assert(!_isInitialized, 'Already initialized.');
    _isInitialized = true;

    final builder = ExpressionBuilder()
      ..primitive(literalConstant)
      // grouping
      ..wrapper(LexerGrammar.LPAREN, LexerGrammar.RPAREN)
      // unary postfix
      // TODO(JonasWanke): add navigation
      ..postfix(LexerGrammar.PLUS_PLUS |
          LexerGrammar.MINUS_MINUS |
          LexerGrammar.QUESTION |
          LexerGrammar.EXCLAMATION)
      ..complexPostfix<List<SyntacticEntity>, InvocationExpression>(
        invocationPostfix,
        mapper: (expression, postfix) {
          return InvocationExpression(
            target: expression,
            leftParenthesis: postfix.first as OperatorToken,
            arguments: postfix.sublist(1, postfix.length - 2) as List<Argument>,
            rightParenthesis: postfix.last as OperatorToken,
          );
        },
      )
      ..complexPostfix<List<SyntacticEntity>, IndexExpression>(
        indexingPostfix,
        mapper: (expression, postfix) {
          return IndexExpression(
            target: expression,
            leftSquareBracket: postfix.first as OperatorToken,
            indices: postfix.sublist(1, postfix.length - 2) as List<Expression>,
            rightSquareBracket: postfix.last as OperatorToken,
          );
        },
      )
      // unary prefix
      ..prefix(LexerGrammar.MINUS |
          LexerGrammar.EXCLAMATION |
          LexerGrammar.TILDE |
          LexerGrammar.PLUS_PLUS |
          LexerGrammar.MINUS_MINUS)
      // implicit multiplication
      // TODO(JonasWanke): add implicit multiplication
      // multiplicative
      ..left(LexerGrammar.ASTERISK |
          LexerGrammar.SLASH |
          LexerGrammar.TILDE_SLASH |
          LexerGrammar.PERCENT)
      // additive
      ..left(LexerGrammar.PLUS | LexerGrammar.MINUS)
      // shift
      ..left(LexerGrammar.LESS_LESS |
          LexerGrammar.GREATER_GREATER |
          LexerGrammar.GREATER_GREATER_GREATER)
      // bitwise and
      ..left(LexerGrammar.AMPERSAND)
      // bitwise or
      ..left(LexerGrammar.CARET)
      // bitwise not
      ..left(LexerGrammar.BAR)
      // type check
      ..left(LexerGrammar.AS | LexerGrammar.AS_SAFE)
      // range
      ..left(LexerGrammar.DOT_DOT | LexerGrammar.DOT_DOT_EQUALS)
      // infix function
      // TODO(JonasWanke): infix function
      // named checks
      ..left(LexerGrammar.IN |
          LexerGrammar.EXCLAMATION_IN |
          LexerGrammar.IS |
          LexerGrammar.EXCLAMATION_IS)
      // comparison
      ..left(LexerGrammar.LESS |
          LexerGrammar.LESS_EQUAL |
          LexerGrammar.GREATER |
          LexerGrammar.GREATER_EQUAL)
      // equality
      ..left(LexerGrammar.EQUALS_EQUALS |
          LexerGrammar.EXCLAMATION_EQUALS_EQUALS |
          LexerGrammar.EQUALS_EQUALS_EQUALS |
          LexerGrammar.EXCLAMATION_EQUALS_EQUALS)
      // logical and
      ..left(LexerGrammar.AMPERSAND_AMPERSAND)
      // logical or
      ..left(LexerGrammar.BAR_BAR)
      // logical implication
      ..left(LexerGrammar.DASH_GREATER | LexerGrammar.LESS_DASH)
      // spread
      ..prefix(LexerGrammar.DOT_DOT_DOT)
      // assignment
      ..right(LexerGrammar.EQUALS |
          LexerGrammar.ASTERISK_EQUALS |
          LexerGrammar.SLASH_EQUALS |
          LexerGrammar.TILDE_SLASH_EQUALS |
          LexerGrammar.PERCENT_EQUALS |
          LexerGrammar.PLUS_EQUALS |
          LexerGrammar.MINUS_EQUALS |
          LexerGrammar.AMPERSAND_EQUALS |
          LexerGrammar.BAR_EQUALS |
          LexerGrammar.CARET_EQUALS |
          LexerGrammar.AMPERSAND_AMPERSAND_EQUALS |
          LexerGrammar.BAR_BAR_EQUALS |
          LexerGrammar.LESS_LESS_EQUALS |
          LexerGrammar.GREATER_GREATER_EQUALS |
          LexerGrammar.GREATER_GREATER_GREATER_EQUALS);

    _expression.set(builder.build());
  }

  static bool _isInitialized = false;

  static final _expression = undefined<dynamic>();
  static Parser<dynamic> get expression => _expression;

  // TODO(JonasWanke): typeArguments? valueArguments? annotatedLambda | typeArguments? valueArguments
  static final invocationPostfix = (LexerGrammar.LPAREN &
          valueArguments &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN)
      .map<List<SyntacticEntity>>((value) {
    return [
      value[0] as OperatorToken, // leftParenthesis
      ...value[1] as List<Argument>, // arguments
      value[3] as OperatorToken, // rightParenthesis
    ];
  });

  static final valueArguments = (LexerGrammar.NLs &
          valueArgument &
          (LexerGrammar.NLs &
                  LexerGrammar.COMMA &
                  LexerGrammar.NLs &
                  valueArgument)
              .map<Argument>((value) => value[3] as Argument)
              .star() &
          (LexerGrammar.NLs & LexerGrammar.COMMA).optional())
      .optional()
      .map<List<Argument>>((value) {
    return [value[1] as Argument, ...value[2] as List<Argument>];
  });

  static final valueArgument = (LexerGrammar.NLs &
          (simpleIdentifier &
                  LexerGrammar.NLs &
                  LexerGrammar.EQUALS &
                  LexerGrammar.NLs)
              .optional() &
          LexerGrammar.NLs &
          expression)
      .map<Argument>((value) {
    return Argument(
      name: (value[1] as List<dynamic>)?.first as SimpleIdentifierToken,
      equals: (value[1] as List<dynamic>)?.elementAt(2) as OperatorToken,
      expression: value[3] as Expression,
    );
  });

  static final indexingPostfix = (LexerGrammar.LSQUARE &
          LexerGrammar.NLs &
          expression &
          (LexerGrammar.NLs &
                  LexerGrammar.COMMA &
                  LexerGrammar.NLs &
                  expression)
              .map<Expression>((v) => v[3] as Expression)
              .star() &
          (LexerGrammar.NLs & LexerGrammar.COMMA).optional() &
          LexerGrammar.NLs &
          LexerGrammar.RSQUARE)
      .map<List<SyntacticEntity>>((value) {
    return [
      value[0] as OperatorToken, // leftSquareBracket
      value[2] as Expression,
      ...value[3] as List<Expression>,
      value[6] as OperatorToken, // rightSquareBracket
    ];
  });

  static final literalConstant =
      // ignore: unnecessary_cast, Without the cast the compiler complains…
      (LexerGrammar.IntegerLiteral.map((l) => Literal<int>(l))
              as Parser<Literal<dynamic>>) |
          LexerGrammar.BooleanLiteral.map((l) => Literal<bool>(l));

  // SECTION: identifiers

  static final simpleIdentifier = LexerGrammar.Identifier;

  // TODO
  static final identifier = simpleIdentifier &
      (LexerGrammar.NLs & LexerGrammar.DOT & simpleIdentifier).star();
}

extension on ExpressionBuilder {
  void primitive(Parser<Expression> primitive) =>
      group().primitive<Expression>(primitive);

  void wrapper(Parser<OperatorToken> left, Parser<OperatorToken> right) {
    group().wrapper<OperatorToken, Expression>(
      left,
      right,
      (left, expression, right) =>
          ParenthesizedExpression(left, expression, right),
    );
  }

  void postfix(Parser<OperatorToken> operator) {
    group().postfix<OperatorToken, Expression>(
      operator,
      (operand, operator) => PostfixExpression(operand, operator),
    );
  }

  void complexPostfix<T, R>(
    Parser<T> postfix, {
    @required R Function(Expression expression, T postfix) mapper,
  }) =>
      group().postfix<T, Expression>(postfix, mapper);

  void prefix(Parser<OperatorToken> operator) {
    group().prefix<OperatorToken, Expression>(
      operator,
      (operator, operand) => PrefixExpression(operator, operand),
    );
  }

  void left(Parser<OperatorToken> operator) {
    group().left<OperatorToken, Expression>(
      operator,
      (left, operator, right) => BinaryExpression(left, operator, right),
    );
  }

  void right(Parser<OperatorToken> operator) {
    group().right<OperatorToken, Expression>(
      operator,
      (left, operator, right) => BinaryExpression(left, operator, right),
    );
  }
}
