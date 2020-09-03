import 'package:meta/meta.dart';
import 'package:petitparser/petitparser.dart'
    hide ChoiceParser, ChoiceParserExtension;

import '../lexer/lexer.dart';
import '../syntactic_entity.dart';
import '../utils.dart';
import 'ast/declarations.dart';
import 'ast/expressions/expression.dart';
import 'ast/statements.dart';
import 'ast/types.dart';

// ignore: avoid_classes_with_only_static_members
@immutable
class ParserGrammar {
  static void init() {
    assert(!_isInitialized, 'Already initialized.');
    _isInitialized = true;

    _initDeclaration();
    _initType();
    _initExpression();
  }

  static bool _isInitialized = false;

  // SECTION: declarations

  static final declarations =
      (declaration & semis.optional()).map((v) => v[0] as Declaration).star();
  static final _declaration = undefined<Declaration>();
  static Parser<Declaration> get declaration => _declaration;
  static void _initDeclaration() {
    // ignore: unnecessary_cast
    _declaration.set((classDeclaration as Parser<Declaration>) |
        functionDeclaration |
        propertyDeclaration);
  }

  static final Parser<ClassDeclaration> classDeclaration = (modifiers
              .optional() &
          LexerGrammar.CLASS &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          (LexerGrammar.NLs &
                  LexerGrammar.COLON &
                  LexerGrammar.NLs &
                  constructorInvocation)
              .optional() &
          (LexerGrammar.NLs & classBody).optional())
      .map<ClassDeclaration>((value) {
    final parentConstructorInvocation = value[4] as List<dynamic>;
    return ClassDeclaration(
      modifiers: value[0] as List<ModifierToken> ?? [],
      classKeyword: value[1] as ClassKeywordToken,
      name: value[3] as IdentifierToken,
      colon: parentConstructorInvocation?.elementAt(1) as OperatorToken,
      parentConstructorInvocation:
          parentConstructorInvocation?.elementAt(3) as ConstructorInvocation,
      body: (value[5] as List<dynamic>)?.elementAt(1) as ClassBody,
    );
  });
  static final classBody = (LexerGrammar.LCURL &
          LexerGrammar.NLs &
          declarations &
          LexerGrammar.NLs &
          LexerGrammar.RCURL)
      .map((value) => ClassBody(
            leftBrace: value[0] as OperatorToken,
            declarations: value[2] as List<Declaration>,
            rightBrace: value[4] as OperatorToken,
          ));
  static final constructorInvocation =
      (userType & invocationPostfix).map<ConstructorInvocation>((value) {
    final invocationPostfix = value[1] as List<dynamic>;
    return ConstructorInvocation(
      type: value[0] as UserType,
      leftParenthesis: invocationPostfix[0] as OperatorToken,
      arguments: invocationPostfix[1] as List<Argument>,
      argumentCommata: invocationPostfix[2] as List<OperatorToken>,
      rightParenthesis: invocationPostfix[3] as OperatorToken,
    );
  });

  static final functionDeclaration = (modifiers.optional() &
          LexerGrammar.FUN &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          LexerGrammar.NLs &
          LexerGrammar.LPAREN &
          LexerGrammar.NLs &
          valueParameter.fullCommaSeparatedList().optional() &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional() &
          // (LexerGrammar.NLs & typeConstraints).optional() &
          (LexerGrammar.NLs & block).optional())
      .map<FunctionDeclaration>((value) {
    final parameterList = value[7] as List<dynamic>;
    final returnTypeDeclaration = value[10] as List<dynamic>;
    return FunctionDeclaration(
      modifiers: (value[0] as List<ModifierToken>) ?? [],
      funKeyword: value[1] as FunKeywordToken,
      name: value[3] as IdentifierToken,
      leftParenthesis: value[5] as OperatorToken,
      valueParameters:
          parameterList?.elementAt(0) as List<ValueParameter> ?? [],
      valueParameterCommata:
          parameterList?.elementAt(1) as List<OperatorToken> ?? [],
      rightParenthesis: value[9] as OperatorToken,
      colon: returnTypeDeclaration?.elementAt(1) as OperatorToken,
      returnType: returnTypeDeclaration?.elementAt(3) as Type,
      body: (value[11] as List<dynamic>)?.elementAt(1) as Block,
    );
  });

  static final propertyDeclaration = (modifiers.optional() &
          LexerGrammar.LET &
          (LexerGrammar.NLs & LexerGrammar.MUT).optional() &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          LexerGrammar.NLs &
          LexerGrammar.COLON &
          LexerGrammar.NLs &
          type &
          (LexerGrammar.NLs &
                  LexerGrammar.EQUALS &
                  LexerGrammar.NLs &
                  expression)
              .optional() &
          (LexerGrammar.NLs & propertyAccessors).optional())
      .map<PropertyDeclaration>((value) {
    final initializerDeclaration = value[9] as List<dynamic>;
    return PropertyDeclaration(
      modifiers: (value[0] as List<ModifierToken>) ?? [],
      letKeyword: value[1] as LetKeywordToken,
      mutKeyword: (value[2] as List<dynamic>)?.elementAt(1) as MutKeywordToken,
      name: value[4] as IdentifierToken,
      colon: value[6] as OperatorToken,
      type: value[8] as Type,
      equals: initializerDeclaration?.elementAt(1) as OperatorToken,
      initializer: initializerDeclaration?.elementAt(3) as Expression,
      accessors: (value[10] as List<dynamic>)?.elementAt(1)
              as List<PropertyAccessor> ??
          [],
    );
  });
  static final propertyAccessors = (propertyAccessor &
          (LexerGrammar.NLs & propertyAccessor)
              .map<PropertyAccessor>((v) => v[1] as PropertyAccessor)
              .star())
      .map((value) => [
            value[0] as PropertyAccessor,
            ...value[1] as List<PropertyAccessor>,
          ]);
  static final propertyAccessor =
      // ignore: unnecessary_cast
      (propertyGetter as Parser<PropertyAccessor>) | propertySetter;
  static final propertyGetter = (LexerGrammar.GET &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional() &
          (LexerGrammar.NLs & block).optional())
      .map<GetterPropertyAccessor>((value) {
    final returnTypeDeclaration = value[1] as List<dynamic>;
    return PropertyAccessor.getter(
      keyword: value[0] as GetKeywordToken,
      colon: returnTypeDeclaration?.elementAt(1) as OperatorToken,
      returnType: returnTypeDeclaration?.elementAt(3) as Type,
      body: (value[2] as List<dynamic>)?.elementAt(1) as Block,
    ) as GetterPropertyAccessor;
  });
  static final propertySetter = (LexerGrammar.SET &
          (LexerGrammar.LPAREN &
                  LexerGrammar.NLs &
                  valueParameter.optional() &
                  LexerGrammar.NLs &
                  (LexerGrammar.COMMA & LexerGrammar.NLs).optional() &
                  LexerGrammar.RPAREN)
              .optional() &
          (LexerGrammar.NLs & block).optional())
      .map<SetterPropertyAccessor>((value) {
    final parameterDeclaration = value[1] as List<dynamic>;
    return PropertyAccessor.setter(
      keyword: value[0] as SetKeywordToken,
      leftParenthesis: parameterDeclaration?.elementAt(0) as OperatorToken,
      valueParameter: parameterDeclaration?.elementAt(2) as ValueParameter,
      valueParameterComma: (parameterDeclaration?.elementAt(4) as List<dynamic>)
          ?.elementAt(0) as OperatorToken,
      rightParenthesis: parameterDeclaration?.elementAt(5) as OperatorToken,
      body: (value[2] as List<dynamic>)?.elementAt(1) as Block,
    ) as SetterPropertyAccessor;
  });

  static final valueParameter = (LexerGrammar.Identifier &
          LexerGrammar.NLs &
          LexerGrammar.COLON &
          LexerGrammar.NLs &
          type &
          (LexerGrammar.NLs &
                  LexerGrammar.EQUALS &
                  LexerGrammar.NLs &
                  expression)
              .optional())
      .map<ValueParameter>((value) {
    final defaultValueDeclaration = value[5] as List<dynamic>;
    return ValueParameter(
      name: value[0] as IdentifierToken,
      colon: value[2] as OperatorToken,
      type: value[4] as Type,
      equals: defaultValueDeclaration?.elementAt(1) as OperatorToken,
      defaultValue: defaultValueDeclaration?.elementAt(3) as Expression,
    );
  });

  // SECTION: types

  static final _type = undefined<Type>();
  static Parser<Type> get type => _type;
  static void _initType() {
    final builder = ExpressionBuilder()
      // ignore: unnecessary_cast
      ..primitive<Type>((userType as Parser<Type>) | groupType | tupleType)
      ..left<Type>(
        LexerGrammar.AMPERSAND,
        mapper: (left, ampersand, right) => IntersectionType(
          leftType: left,
          ampersand: ampersand,
          rightType: right,
        ),
      )
      ..left<Type>(
        LexerGrammar.BAR,
        mapper: (left, bar, right) => UnionType(
          leftType: left,
          bar: bar,
          rightType: right,
        ),
      )
      ..prefix<List<dynamic>, Type>(
        functionTypePrefix,
        mapper: (operator, type) => FunctionType(
          receiver: operator[0] as Type,
          receiverDot: operator[1] as OperatorToken,
          leftParenthesis: operator[2] as OperatorToken,
          parameterTypes: operator[3] as List<Type>,
          parameterCommata: operator[4] as List<OperatorToken>,
          rightParenthesis: operator[5] as OperatorToken,
          arrow: operator[6] as OperatorToken,
          returnType: type,
        ),
      );

    _type.set(builder.build().map((dynamic t) => t as Type));
  }

  static final userType =
      simpleUserType.separatedList(LexerGrammar.DOT).map((values) => UserType(
            simpleTypes: values.first as List<SimpleUserType>,
            dots: values[1] as List<OperatorToken>,
          ));
  static final Parser<SimpleUserType> simpleUserType =
      LexerGrammar.Identifier.map($SimpleUserType);

  static final groupType = (LexerGrammar.LPAREN &
          LexerGrammar.NLs &
          type &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN)
      .map((value) => GroupType(
            leftParenthesis: value[0] as OperatorToken,
            type: value[2] as Type,
            rightParenthesis: value[4] as OperatorToken,
          ));

  static final functionTypePrefix = ((functionTypeReceiver &
                  LexerGrammar.NLs &
                  LexerGrammar.DOT &
                  LexerGrammar.NLs)
              .optional() &
          functionTypeParameters &
          LexerGrammar.NLs &
          LexerGrammar.EQUALS_GREATER &
          LexerGrammar.NLs)
      .map<List<dynamic>>((value) {
    final receiverPrefix = value[0] as List<dynamic>;
    final parameters = value[1] as List<dynamic>;
    return <dynamic>[
      receiverPrefix?.elementAt(0), // receiver
      receiverPrefix?.elementAt(2), // receiverDot
      parameters[0] as OperatorToken, // leftParenthesis
      parameters[1] as List<Type>, // parameterTypes
      parameters[2] as List<OperatorToken>, // parameterCommata
      parameters[3] as OperatorToken, // rightParenthesis
      value[3] as OperatorToken, // arrow
    ];
  });
  static final functionTypeParameters = (LexerGrammar.LPAREN &
          LexerGrammar.NLs &
          type.fullCommaSeparatedList().optional() &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN)
      .map((value) => <dynamic>[
            value[0] as OperatorToken, // leftParenthesis
            value[2]?.elementAt(0) as List<Type> ?? <Type>[], // types
            value[2]?.elementAt(1) as List<OperatorToken> ??
                <OperatorToken>[], // commata
            value[4] as OperatorToken, // rightParenthesis
          ]);
  // ignore: unnecessary_cast
  static final functionTypeReceiver = (userType as Parser<Type>) | groupType;

  static final tupleType = (LexerGrammar.LPAREN &
          LexerGrammar.NLs &
          type.fullCommaSeparatedList(2) &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN)
      .map((value) => TupleType(
            leftParenthesis: value[0] as OperatorToken,
            types: value[2][0] as List<Type>,
            commata: value[2][1] as List<OperatorToken>,
            rightParenthesis: value[4] as OperatorToken,
          ));

  // SECTION: statements

  static final Parser<List<Statement>> statements =
      ((statement & (semis & statement).map((v) => v[1] as Statement).star())
                  .map((v) => [v[0] as Statement, ...v[1] as List<Statement>])
                  .optional() &
              semis.optional())
          .map((values) => values[0] as List<Statement> ?? []);
  static final Parser<Statement> statement =
      expression.map($Statement.expression);

  static final block = (LexerGrammar.LCURL &
          LexerGrammar.NLs &
          statements &
          LexerGrammar.NLs &
          LexerGrammar.RCURL)
      .map((values) => Block(
            leftBrace: values.first as OperatorToken,
            statements: values[2] as List<Statement>,
            rightBrace: values[4] as OperatorToken,
          ));

  // ignore: unnecessary_cast
  static final Parser<void> semi = (LexerGrammar.WS.optional() &
          (LexerGrammar.SEMICOLON | LexerGrammar.NL) &
          LexerGrammar.NLs as Parser<void>) |
      endOfInput();
  static final Parser<void> semis =
      // ignore: unnecessary_cast
      ((LexerGrammar.WS.optional() &
                  (LexerGrammar.SEMICOLON | LexerGrammar.NL) &
                  LexerGrammar.WS.optional())
              .plus() as Parser<void>) |
          endOfInput();

  // SECTION: expressions

  static final _expression = undefined<Expression>();
  static Parser<Expression> get expression => _expression;
  static void _initExpression() {
    final builder = ExpressionBuilder()
      ..primitive<Expression>(
          // ignore: unnecessary_cast, Without the cast the compiler complains…
          (literalConstant as Parser<Expression>) |
              LexerGrammar.Identifier.map((t) => Identifier(t)))
      // grouping
      ..grouping<Expression>(
        LexerGrammar.LPAREN,
        LexerGrammar.RPAREN,
        mapper: (left, expression, right) => GroupExpression(
          leftParenthesis: left,
          expression: expression,
          rightParenthesis: right,
        ),
      )
      // unary postfix
      ..postfix(LexerGrammar.PLUS_PLUS |
          LexerGrammar.MINUS_MINUS |
          LexerGrammar.QUESTION |
          LexerGrammar.EXCLAMATION)
      ..complexPostfix<List<SyntacticEntity>, NavigationExpression>(
        navigationPostfix,
        mapper: (expression, postfix) {
          return NavigationExpression(
            target: expression,
            dot: postfix.first as OperatorToken,
            name: postfix[1] as IdentifierToken,
          );
        },
      )
      ..complexPostfix<List<dynamic>, CallExpression>(
        invocationPostfix,
        mapper: (expression, postfix) {
          return CallExpression(
            target: expression,
            leftParenthesis: postfix.first as OperatorToken,
            arguments: postfix.sublist(1, postfix.length - 1).cast<Argument>(),
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
      ..prefixExpression(LexerGrammar.EXCLAMATION |
          LexerGrammar.TILDE |
          LexerGrammar.PLUS_PLUS |
          LexerGrammar.MINUS_MINUS |
          LexerGrammar.MINUS)
      // implicit multiplication
      // TODO(JonasWanke): add implicit multiplication
      // multiplicative
      ..leftExpression(LexerGrammar.ASTERISK |
          LexerGrammar.SLASH |
          LexerGrammar.TILDE_SLASH |
          LexerGrammar.PERCENT)
      // additive
      ..leftExpression(LexerGrammar.PLUS | LexerGrammar.MINUS)
      // shift
      ..leftExpression(LexerGrammar.LESS_LESS |
          LexerGrammar.GREATER_GREATER |
          LexerGrammar.GREATER_GREATER_GREATER)
      // bitwise and
      ..leftExpression(LexerGrammar.AMPERSAND)
      // bitwise or
      ..leftExpression(LexerGrammar.CARET)
      // bitwise not
      ..leftExpression(LexerGrammar.BAR)
      // type check
      ..leftExpression(LexerGrammar.AS | LexerGrammar.AS_SAFE)
      // range
      ..leftExpression(LexerGrammar.DOT_DOT | LexerGrammar.DOT_DOT_EQUALS)
      // infix function
      // TODO(JonasWanke): infix function
      // named checks
      ..leftExpression(LexerGrammar.IN |
          LexerGrammar.EXCLAMATION_IN |
          LexerGrammar.IS |
          LexerGrammar.EXCLAMATION_IS)
      // comparison
      ..leftExpression(LexerGrammar.LESS |
          LexerGrammar.LESS_EQUAL |
          LexerGrammar.GREATER |
          LexerGrammar.GREATER_EQUAL)
      // equality
      ..leftExpression(LexerGrammar.EQUALS_EQUALS |
          LexerGrammar.EXCLAMATION_EQUALS_EQUALS |
          LexerGrammar.EQUALS_EQUALS_EQUALS |
          LexerGrammar.EXCLAMATION_EQUALS_EQUALS)
      // logical and
      ..leftExpression(LexerGrammar.AMPERSAND_AMPERSAND)
      // logical or
      ..leftExpression(LexerGrammar.BAR_BAR)
      // logical implication
      ..leftExpression(LexerGrammar.DASH_GREATER | LexerGrammar.LESS_DASH)
      // spread
      ..prefixExpression(LexerGrammar.DOT_DOT_DOT)
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

    _expression.set(builder.build().map((dynamic e) => e as Expression));
  }

  static final navigationPostfix = (LexerGrammar.NLs &
          LexerGrammar.DOT &
          LexerGrammar.NLs &
          LexerGrammar.Identifier)
      .map<List<SyntacticEntity>>((value) {
    return [
      value[1] as OperatorToken, // dot
      value[3] as IdentifierToken, // name
    ];
  });

  // TODO(JonasWanke): typeArguments? valueArguments? annotatedLambda | typeArguments? valueArguments
  static final invocationPostfix = (LexerGrammar.LPAREN &
          valueArguments &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN)
      .map<List<dynamic>>((value) {
    final arguments = value[1] as List<dynamic>;
    return <dynamic>[
      value[0] as OperatorToken, // leftParenthesis
      arguments[0] as List<Argument>, // arguments
      arguments[1] as List<OperatorToken>, // argumentCommata
      value[3] as OperatorToken, // rightParenthesis
    ];
  });

  static final valueArguments = (LexerGrammar.NLs &
          valueArgument.fullCommaSeparatedList().optional() &
          LexerGrammar.NLs)
      .map((value) => value[1] as List<dynamic> ?? <dynamic>[]);

  static final valueArgument = (LexerGrammar.NLs &
          (LexerGrammar.Identifier &
                  LexerGrammar.NLs &
                  LexerGrammar.EQUALS &
                  LexerGrammar.NLs)
              .optional() &
          LexerGrammar.NLs &
          expression)
      .map<Argument>((value) {
    return Argument(
      name: (value[1] as List<dynamic>)?.first as IdentifierToken,
      equals: (value[1] as List<dynamic>)?.elementAt(2) as OperatorToken,
      expression: value[3] as Expression,
    );
  });

  static final indexingPostfix = (LexerGrammar.LSQUARE &
          LexerGrammar.NLs &
          expression.commaSeparatedList() &
          LexerGrammar.NLs &
          LexerGrammar.RSQUARE)
      .map<List<SyntacticEntity>>((value) {
    return [
      value[0] as OperatorToken, // leftSquareBracket
      ...value[2] as List<Expression>, // indices
      value[4] as OperatorToken, // rightSquareBracket
    ];
  });

  static final literalConstant = ChoiceParser<Literal<dynamic>>([
    LexerGrammar.IntegerLiteral.map((l) => Literal<int>(l)),
    LexerGrammar.BooleanLiteral.map((l) => Literal<bool>(l)),
  ]);

  // SECTION: modifiers

  static final modifiers = modifier.plus();
  static final modifier =
      ((LexerGrammar.EXTERNAL | LexerGrammar.ABSTRACT | LexerGrammar.CONST) &
              LexerGrammar.NLs)
          .map((value) => value[0] as ModifierToken);
}

extension<T> on Parser<T> {
  Parser<List<dynamic>> separatedList(Parser<OperatorToken> separator) {
    return (this &
            (LexerGrammar.NLs & separator & LexerGrammar.NLs & this)
                .map<dynamic>((v) => [v[1] as OperatorToken, v[3] as T])
                .star())
        .map((value) {
      final trailing = (value[1] as List<dynamic>).cast<List<dynamic>>();
      return <dynamic>[
        [value.first as T, ...trailing.map((dynamic v) => v[1] as T)],
        [...trailing.map((dynamic v) => v[0] as OperatorToken)],
      ];
    });
  }

  Parser<List<dynamic>> fullCommaSeparatedList([int minimum = 1]) {
    assert(minimum != null);
    assert(minimum >= 1);

    return (this &
            (LexerGrammar.NLs & LexerGrammar.COMMA & LexerGrammar.NLs & this)
                .map<dynamic>((v) => [v[1] as OperatorToken, v[3] as T])
                .repeat(minimum - 1, unbounded) &
            (LexerGrammar.NLs & LexerGrammar.COMMA).optional())
        .map((value) {
      final trailing = (value[1] as List<dynamic>).cast<List<dynamic>>();
      final trailingComma =
          (value[2] as List<dynamic>)?.elementAt(1) as OperatorToken;
      return <dynamic>[
        [value.first as T, ...trailing.map((dynamic v) => v[1] as T)],
        [
          ...trailing.map((dynamic v) => v[0] as OperatorToken),
          if (trailingComma != null) trailingComma,
        ],
      ];
    });
  }

  Parser<List<T>> commaSeparatedList() {
    return (this &
            (LexerGrammar.NLs & LexerGrammar.COMMA & LexerGrammar.NLs & this)
                .map<T>((v) => v[3] as T)
                .star() &
            (LexerGrammar.NLs & LexerGrammar.COMMA).optional())
        .map((value) {
      return [value.first as T, ...value[1] as List<T>];
    });
  }
}

extension on ExpressionBuilder {
  void primitive<T>(Parser<T> primitive) => group().primitive<T>(primitive);

  void grouping<T>(
    Parser<OperatorToken> left,
    Parser<OperatorToken> right, {
    @required T Function(OperatorToken, T, OperatorToken) mapper,
  }) =>
      group().wrapper<OperatorToken, T>(left, right, mapper);

  void postfix(Parser<OperatorToken> operator) {
    group().postfix<OperatorToken, Expression>(
      operator,
      (operand, operator) =>
          PostfixExpression(operand: operand, operatorToken: operator),
    );
  }

  void complexPostfix<T, R>(
    Parser<T> postfix, {
    @required R Function(Expression expression, T postfix) mapper,
  }) =>
      group().postfix<T, Expression>(postfix, mapper);

  void prefix<O, T>(Parser<O> operator, {@required T Function(O, T) mapper}) {
    group().prefix<O, T>(operator, mapper);
  }

  void prefixExpression(Parser<OperatorToken> operator) {
    prefix<OperatorToken, Expression>(
      operator,
      mapper: (operator, operand) =>
          PrefixExpression(operatorToken: operator, operand: operand),
    );
  }

  void left<T>(
    Parser<OperatorToken> operator, {
    @required T Function(T, OperatorToken, T) mapper,
  }) =>
      group().left<List<dynamic>, T>(
        LexerGrammar.WS.optional() & operator & LexerGrammar.NLs,
        (left, operator, right) =>
            mapper(left, operator[1] as OperatorToken, right),
      );

  void leftExpression(Parser<OperatorToken> operator) {
    left<Expression>(
      operator,
      mapper: (left, operator, right) =>
          BinaryExpression(left, operator, right),
    );
  }

  void right(Parser<OperatorToken> operator) {
    group().right<OperatorToken, Expression>(
      operator,
      (left, operator, right) => BinaryExpression(left, operator, right),
    );
  }
}
