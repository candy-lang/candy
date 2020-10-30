import 'package:meta/meta.dart';
import 'package:petitparser/petitparser.dart'
    hide ChoiceParser, ChoiceParserExtension;

import '../lexer/lexer.dart';
import '../syntactic_entity.dart';
import '../utils.dart';
import 'ast/declarations.dart';
import 'ast/expressions/expressions.dart';
import 'ast/general.dart';
import 'ast/types.dart';

// ignore: avoid_classes_with_only_static_members
@immutable
class ParserGrammar {
  static CandyFile parse(String fileNameWithoutExtension, String source) {
    init();
    return _candyFile(fileNameWithoutExtension).parse(source).value;
  }

  @visibleForTesting
  static void init() {
    _id = 0;
    if (_isInitialized) return;
    _isInitialized = true;

    LexerGrammar.init();

    _initDeclaration();
    _initType();
    _initExpression();
  }

  // This is really ugly. Fortunately, we'll rewrite the whole thing in Candy
  // soon.
  static var _id = 0;

  static bool _isInitialized = false;

  // SECTION: general

  static Parser<CandyFile> _candyFile(String fileNameWithoutExtension) {
    return (useLines & LexerGrammar.NLs & declarations)
        .end()
        .map((value) => CandyFile(
              useLines: value[0] as List<UseLine>,
              declaration: ModuleDeclaration(
                moduleKeyword: ModuleKeywordToken(),
                name: IdentifierToken(fileNameWithoutExtension),
                body: BlockDeclarationBody(
                  leftBrace: OperatorToken(OperatorTokenType.lcurl),
                  declarations: value[2] as List<Declaration>,
                  rightBrace: OperatorToken(OperatorTokenType.rcurl),
                ),
              ),
            ));
  }

  static final useLines =
      (useLine & semi).map<UseLine>((v) => v[0] as UseLine).star();
  // ignore: unnecessary_cast
  static final useLine = (localAbsoluteUseLine as Parser<UseLine>) |
      localRelativeUseLine |
      globalUseLine;
  static final localAbsoluteUseLine = (modifiers.optional() &
          LexerGrammar.USE &
          LexerGrammar.NLs &
          LexerGrammar.CRATE &
          LexerGrammar.NLs &
          LexerGrammar.DOT &
          LexerGrammar.NLs &
          LexerGrammar.Identifier.separatedList(LexerGrammar.DOT))
      .map<UseLine>((value) {
    final path = value[7] as List<dynamic>;
    return UseLine.localAbsolute(
      modifiers: value[0] as List<ModifierToken> ?? [],
      useKeyword: value[1] as UseKeywordToken,
      crateKeyword: value[3] as CrateKeywordToken,
      dots: [value[5] as OperatorToken, ...path[1] as List<OperatorToken>],
      pathSegments: path[0] as List<IdentifierToken>,
    );
  });
  static final localRelativeUseLine = (modifiers.optional() &
          LexerGrammar.USE &
          LexerGrammar.NLs &
          LexerGrammar.DOT.plus() &
          LexerGrammar.NLs &
          LexerGrammar.Identifier.separatedList(LexerGrammar.DOT))
      .map<UseLine>((value) {
    final path = value[5] as List<dynamic>;
    return UseLine.localRelative(
      modifiers: value[0] as List<ModifierToken> ?? [],
      useKeyword: value[1] as UseKeywordToken,
      leadingDots: value[3] as List<OperatorToken>,
      pathSegments: path[0] as List<IdentifierToken>,
      dots: path[1] as List<OperatorToken>,
    );
  });
  static final globalUseLine = (modifiers.optional() &
          LexerGrammar.USE &
          LexerGrammar.NLs &
          LexerGrammar.Identifier.separatedList(LexerGrammar.SLASH) &
          (LexerGrammar.NLs &
                  LexerGrammar.DOT &
                  LexerGrammar.NLs &
                  LexerGrammar.Identifier)
              .optional())
      .map<UseLine>((value) {
    final packagePart = value[3] as List<dynamic>;
    final modulePart = value[4] as List<dynamic>;
    return UseLine.global(
      modifiers: value[0] as List<ModifierToken> ?? [],
      useKeyword: value[1] as UseKeywordToken,
      packagePathSegments: packagePart[0] as List<IdentifierToken>,
      slashes: packagePart[1] as List<OperatorToken>,
      dot: modulePart?.elementAt(1) as OperatorToken,
      moduleName: modulePart?.elementAt(3) as IdentifierToken,
    );
  });

  // SECTION: declarations

  static final declarations =
      (declaration & semis.optional()).map((v) => v[0] as Declaration).star();
  static final _declaration = undefined<Declaration>();
  static Parser<Declaration> get declaration => _declaration;
  static void _initDeclaration() {
    // ignore: unnecessary_cast
    _declaration.set((moduleDeclaration as Parser<Declaration>) |
        traitDeclaration |
        implDeclaration |
        classDeclaration |
        functionDeclaration |
        propertyDeclaration);
  }

  static final moduleDeclaration = (modifiers.optional() &
          LexerGrammar.MODULE &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          (LexerGrammar.NLs & blockDeclarationBody).optional())
      .map((value) => ModuleDeclaration(
            modifiers: value[0] as List<ModifierToken> ?? [],
            moduleKeyword: value[1] as ModuleKeywordToken,
            name: value[3] as IdentifierToken,
            body: (value[4] as List<dynamic>)?.elementAt(1)
                as BlockDeclarationBody,
          ));
  static final traitDeclaration = (modifiers.optional() &
          LexerGrammar.TRAIT &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          (LexerGrammar.NLs & typeParameters).optional() &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional() &
          (LexerGrammar.NLs & blockDeclarationBody).optional())
      .map<TraitDeclaration>((value) {
    final bound = value[5] as List<dynamic>;
    return TraitDeclaration(
      modifiers: value[0] as List<ModifierToken> ?? [],
      traitKeyword: value[1] as TraitKeywordToken,
      name: value[3] as IdentifierToken,
      typeParameters:
          (value[4] as List<dynamic>)?.elementAt(1) as TypeParameters,
      colon: bound?.elementAt(1) as OperatorToken,
      bound: bound?.elementAt(3) as Type,
      body: (value[6] as List<dynamic>)?.elementAt(1) as BlockDeclarationBody,
    );
  });
  static final implDeclaration = (modifiers.optional() &
          LexerGrammar.IMPL &
          (LexerGrammar.NLs & typeParameters).optional() &
          LexerGrammar.NLs &
          type &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional() &
          (LexerGrammar.NLs & blockDeclarationBody).optional())
      .map<ImplDeclaration>((value) {
    final trait = value[5] as List<dynamic>;
    return ImplDeclaration(
      modifiers: value[0] as List<ModifierToken> ?? [],
      implKeyword: value[1] as ImplKeywordToken,
      typeParameters:
          (value[2] as List<dynamic>)?.elementAt(1) as TypeParameters,
      type: value[4] as Type,
      colon: trait?.elementAt(1) as OperatorToken,
      trait: trait?.elementAt(3) as Type,
      body: (value[6] as List<dynamic>)?.elementAt(1) as BlockDeclarationBody,
    );
  });
  static final classDeclaration = (modifiers.optional() &
          LexerGrammar.CLASS &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          (LexerGrammar.NLs & typeParameters).optional() &
          (LexerGrammar.NLs & blockDeclarationBody).optional())
      .map((value) => ClassDeclaration(
            modifiers: value[0] as List<ModifierToken> ?? [],
            classKeyword: value[1] as ClassKeywordToken,
            name: value[3] as IdentifierToken,
            typeParameters:
                (value[4] as List<dynamic>)?.elementAt(1) as TypeParameters,
            body: (value[5] as List<dynamic>)?.elementAt(1)
                as BlockDeclarationBody,
          ));

  static final blockDeclarationBody = (LexerGrammar.LCURL &
          LexerGrammar.NLs &
          declarations &
          LexerGrammar.NLs &
          LexerGrammar.RCURL)
      .map((value) => BlockDeclarationBody(
            leftBrace: value[0] as OperatorToken,
            declarations: value[2] as List<Declaration>,
            rightBrace: value[4] as OperatorToken,
          ));

  static final constructorCall =
      (userType & callPostfix).map<ConstructorCall>((value) {
    final callPostfix = value[1] as List<dynamic>;
    return ConstructorCall(
      type: value[0] as UserType,
      leftParenthesis: callPostfix[0] as OperatorToken,
      arguments: callPostfix[1] as List<Argument>,
      argumentCommata: callPostfix[2] as List<OperatorToken>,
      rightParenthesis: callPostfix[3] as OperatorToken,
    );
  });

  static final functionDeclaration = (modifiers.optional() &
          LexerGrammar.FUN &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          (LexerGrammar.NLs & typeParameters).optional() &
          LexerGrammar.NLs &
          LexerGrammar.LPAREN &
          LexerGrammar.NLs &
          valueParameter.fullCommaSeparatedList().optional() &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional() &
          // (LexerGrammar.NLs & typeConstraints).optional() &
          (LexerGrammar.NLs & lambdaLiteral).optional())
      .map<FunctionDeclaration>((value) {
    final parameterList = value[8] as List<dynamic>;
    final returnTypeDeclaration = value[11] as List<dynamic>;
    return FunctionDeclaration(
      modifiers: (value[0] as List<ModifierToken>) ?? [],
      funKeyword: value[1] as FunKeywordToken,
      name: value[3] as IdentifierToken,
      typeParameters:
          (value[4] as List<dynamic>)?.elementAt(1) as TypeParameters,
      leftParenthesis: value[6] as OperatorToken,
      valueParameters:
          parameterList?.elementAt(0) as List<ValueParameter> ?? [],
      valueParameterCommata:
          parameterList?.elementAt(1) as List<OperatorToken> ?? [],
      rightParenthesis: value[10] as OperatorToken,
      colon: returnTypeDeclaration?.elementAt(1) as OperatorToken,
      returnType: returnTypeDeclaration?.elementAt(3) as Type,
      body: (value[12] as List<dynamic>)?.elementAt(1) as LambdaLiteral,
    );
  });

  static final propertyDeclaration = (modifiers.optional() &
          LexerGrammar.LET &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional() &
          (LexerGrammar.NLs &
                  LexerGrammar.EQUALS &
                  LexerGrammar.NLs &
                  expression)
              .optional() &
          (LexerGrammar.NLs & propertyAccessors).optional())
      .map<PropertyDeclaration>((value) {
    final typeDeclaration = value[4] as List<dynamic>;
    final initializerDeclaration = value[5] as List<dynamic>;
    return PropertyDeclaration(
      _id++,
      modifiers: (value[0] as List<ModifierToken>) ?? [],
      letKeyword: value[1] as LetKeywordToken,
      name: value[3] as IdentifierToken,
      colon: typeDeclaration?.elementAt(1) as OperatorToken,
      type: typeDeclaration?.elementAt(3) as Type,
      equals: initializerDeclaration?.elementAt(1) as OperatorToken,
      initializer: initializerDeclaration?.elementAt(3) as Expression,
      accessors:
          (value[6] as List<dynamic>)?.elementAt(1) as List<PropertyAccessor> ??
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
          (LexerGrammar.NLs & lambdaLiteral).optional())
      .map((value) => PropertyAccessor.getter(
            keyword: value[0] as GetKeywordToken,
            body: (value[1] as List<dynamic>)?.elementAt(1) as LambdaLiteral,
          ) as GetterPropertyAccessor);
  static final propertySetter = (LexerGrammar.SET &
          (LexerGrammar.NLs & lambdaLiteral).optional())
      .map<SetterPropertyAccessor>((value) => PropertyAccessor.setter(
            keyword: value[0] as SetKeywordToken,
            body: (value[1] as List<dynamic>)?.elementAt(1) as LambdaLiteral,
          ) as SetterPropertyAccessor);

  static final typeParameters = (LexerGrammar.LANGLE &
          LexerGrammar.NLs &
          typeParameter.fullCommaSeparatedList() &
          LexerGrammar.NLs &
          LexerGrammar.RANGLE)
      .map<TypeParameters>((value) {
    final parameters = value[2] as List<dynamic>;
    return TypeParameters(
      leftAngle: value[0] as OperatorToken,
      parameters: parameters[0] as List<TypeParameter>,
      commata: parameters[1] as List<OperatorToken>,
      rightAngle: value[4] as OperatorToken,
    );
  });
  static final typeParameter = (modifiers.optional() &
          LexerGrammar.NLs &
          LexerGrammar.Identifier &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional())
      .map<TypeParameter>((value) {
    final boundBlock = value[3] as List<dynamic>;
    return TypeParameter(
      modifiers: value[0] as List<ModifierToken> ?? [],
      name: value[2] as IdentifierToken,
      colon: boundBlock?.elementAt(1) as OperatorToken,
      bound: boundBlock?.elementAt(3) as Type,
    );
  });

  static final typeArguments = (LexerGrammar.LANGLE &
          LexerGrammar.NLs &
          typeArgument.fullCommaSeparatedList() &
          LexerGrammar.NLs &
          LexerGrammar.RANGLE)
      .map<TypeArguments>((value) {
    final arguments = value[2] as List<dynamic>;
    return TypeArguments(
      leftAngle: value[0] as OperatorToken,
      arguments: arguments[0] as List<TypeArgument>,
      commata: arguments[1] as List<OperatorToken>,
      rightAngle: value[4] as OperatorToken,
    );
  });
  static final typeArgument = (modifiers.optional() & LexerGrammar.NLs & type)
      .map((value) => TypeArgument(
            modifiers: value[0] as List<ModifierToken> ?? [],
            type: value[2] as Type,
          ));

  static final valueParameter = (LexerGrammar.Identifier &
          (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
              .optional() &
          (LexerGrammar.NLs &
                  LexerGrammar.EQUALS &
                  LexerGrammar.NLs &
                  expression)
              .optional())
      .map<ValueParameter>((value) {
    final typeDeclaration = value[1] as List<dynamic>;
    final defaultValueDeclaration = value[2] as List<dynamic>;
    return ValueParameter(
      _id++,
      name: value[0] as IdentifierToken,
      colon: typeDeclaration?.elementAt(1) as OperatorToken,
      type: typeDeclaration?.elementAt(3) as Type,
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

  static final userType = (simpleUserType.separatedList(LexerGrammar.DOT) &
          LexerGrammar.NLs &
          typeArguments.optional())
      .map<UserType>((value) {
    final simpleTypes = value[0] as List<dynamic>;
    return UserType(
      simpleTypes: simpleTypes[0] as List<SimpleUserType>,
      dots: simpleTypes[1] as List<OperatorToken>,
      arguments: value[2] as TypeArguments,
    );
  });
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

  // SECTION: expressions

  static final _expression = undefined<Expression>();
  static Parser<Expression> get expression => _expression;
  static void _initExpression() {
    final builder = ExpressionBuilder()
      ..primitive<Expression>(
          // ignore: unnecessary_cast, Without the cast the compiler complains…
          (literalConstant as Parser<Expression>) |
              LexerGrammar.RETURN.map(
                  (value) => ReturnExpression(_id++, returnKeyword: value)) |
              LexerGrammar.BREAK
                  .map((value) => BreakExpression(_id++, breakKeyword: value)) |
              LexerGrammar.CONTINUE.map((value) =>
                  ContinueExpression(_id++, continueKeyword: value)) |
              stringLiteral |
              lambdaLiteral |
              LexerGrammar.Identifier.map((t) => Identifier(_id++, t)))
      // grouping
      ..grouping<Expression>(
        LexerGrammar.LPAREN,
        LexerGrammar.RPAREN,
        mapper: (left, expression, right) => GroupExpression(
          _id++,
          leftParenthesis: left,
          expression: expression,
          rightParenthesis: right,
        ),
      )
      // unary postfix
      ..postfixExpression(LexerGrammar.PLUS_PLUS |
          LexerGrammar.MINUS_MINUS |
          LexerGrammar.QUESTION |
          LexerGrammar.EXCLAMATION);
    builder.group()
      ..postfix<List<SyntacticEntity>, Expression>(
        navigationPostfix,
        (expression, postfix) {
          return NavigationExpression(
            _id++,
            target: expression,
            dot: postfix.first as OperatorToken,
            name: postfix[1] as IdentifierToken,
          );
        },
      )
      ..postfix<List<dynamic>, Expression>(
        typeArguments.optional() & callPostfix,
        (expression, postfix) {
          final valueArguments = postfix[1] as List<dynamic>;
          return CallExpression(
            _id++,
            target: expression,
            typeArguments: postfix[0] as TypeArguments,
            leftParenthesis: valueArguments[0] as OperatorToken,
            arguments: valueArguments[1] as List<Argument> ?? [],
            argumentCommata: valueArguments[2] as List<OperatorToken> ?? [],
            rightParenthesis: valueArguments[3] as OperatorToken,
          );
        },
      )
      ..postfix<List<SyntacticEntity>, Expression>(
        indexingPostfix,
        (expression, postfix) {
          return IndexExpression(
            _id++,
            target: expression,
            leftSquareBracket: postfix.first as OperatorToken,
            indices: postfix.sublist(1, postfix.length - 2) as List<Expression>,
            rightSquareBracket: postfix.last as OperatorToken,
          );
        },
      );
    builder
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
      ..leftComplex<OperatorToken, Expression>(
        (LexerGrammar.WS &
                (LexerGrammar.AS | LexerGrammar.AS_SAFE) &
                LexerGrammar.WS)
            .map((it) => it[1] as OperatorToken),
        mapper: (left, operator, right) =>
            BinaryExpression(_id++, left, operator, right),
      )
      // range
      ..leftExpression(LexerGrammar.DOT_DOT | LexerGrammar.DOT_DOT_EQUALS)
      // infix function
      // TODO(JonasWanke): infix function
      // named checks
      ..postfix<List<dynamic>, Expression>(
        LexerGrammar.NLs &
            (LexerGrammar.IS | LexerGrammar.EXCLAMATION_IS) &
            LexerGrammar.NLs &
            type,
        mapper: (instance, postfix) => IsExpression(
          _id++,
          instance: instance,
          isOperator: postfix[1] as OperatorToken,
          type: postfix[3] as Type,
        ),
      )
      ..leftExpression(LexerGrammar.IN | LexerGrammar.EXCLAMATION_IN)
      // comparison
      ..leftExpression(LexerGrammar.LESS_EQUAL |
          LexerGrammar.LESS |
          LexerGrammar.GREATER_EQUAL |
          LexerGrammar.GREATER)
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
          LexerGrammar.GREATER_GREATER_GREATER_EQUALS)
      // conditional
      ..complexGrouping<List<dynamic>, Expression, List<dynamic>>(
        LexerGrammar.IF & LexerGrammar.NLs,
        LexerGrammar.NLs &
            expression &
            (LexerGrammar.NLs &
                    LexerGrammar.ELSE &
                    LexerGrammar.NLs &
                    expression)
                .optional(),
        mapper: (left, value, right) {
          final elsePart = right[2] as List<dynamic>;
          return IfExpression(
            _id++,
            ifKeyword: left[0] as IfKeywordToken,
            condition: value,
            thenBody: right[1] as LambdaLiteral,
            elseKeyword: elsePart?.elementAt(1) as ElseKeywordToken,
            elseBody: elsePart?.elementAt(3) as LambdaLiteral,
          );
        },
      )
      ..prefix<LoopKeywordToken, Expression>(
          (LexerGrammar.LOOP & LexerGrammar.NLs).map((value) => value.first as LoopKeywordToken),
          mapper: (keyword, body) {
        return LoopExpression(
          _id++,
          loopKeyword: keyword,
          body: body as LambdaLiteral,
        );
      })
      ..complexGrouping<List<dynamic>, Expression, List<dynamic>>(
        LexerGrammar.WHILE & LexerGrammar.NLs,
        LexerGrammar.NLs & expression,
        mapper: (left, value, right) {
          return WhileExpression(
            _id++,
            whileKeyword: left[0] as WhileKeywordToken,
            condition: value,
            body: right[1] as LambdaLiteral,
          );
        },
      )
      ..prefix<ReturnKeywordToken, Expression>(
          (LexerGrammar.RETURN & LexerGrammar.NLs).map((value) => value.first as ReturnKeywordToken),
          mapper: (keyword, expression) {
        return ReturnExpression(
          _id++,
          returnKeyword: keyword,
          expression: expression,
        );
      })
      ..prefix<BreakKeywordToken, Expression>(
          (LexerGrammar.BREAK & LexerGrammar.NLs).map((value) => value.first as BreakKeywordToken),
          mapper: (keyword, expression) {
        return BreakExpression(
          _id++,
          breakKeyword: keyword,
          expression: expression,
        );
      })
      ..prefix<ThrowKeywordToken, Expression>(
          (LexerGrammar.THROW & LexerGrammar.NLs).map((value) => value.first as ThrowKeywordToken), mapper: (keyword, expression) {
        return ThrowExpression(
          _id++,
          throwKeyword: keyword,
          error: expression,
        );
      })
      ..prefix<List<dynamic>, Expression>(
        modifiers.optional() &
            LexerGrammar.LET &
            LexerGrammar.NLs &
            LexerGrammar.Identifier &
            (LexerGrammar.NLs & LexerGrammar.COLON & LexerGrammar.NLs & type)
                .optional() &
            LexerGrammar.NLs &
            LexerGrammar.EQUALS &
            LexerGrammar.NLs,
        mapper: (prefix, expression) {
          final typeDeclaration = prefix[4] as List<dynamic>;
          return PropertyDeclaration(
            _id++,
            modifiers: (prefix[0] as List<ModifierToken>) ?? [],
            letKeyword: prefix[1] as LetKeywordToken,
            name: prefix[3] as IdentifierToken,
            colon: typeDeclaration?.elementAt(1) as OperatorToken,
            type: typeDeclaration?.elementAt(3) as Type,
            equals: prefix[6] as OperatorToken,
            initializer: expression,
          );
        },
      );

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
  static final callPostfix = (LexerGrammar.LPAREN &
          valueArguments &
          LexerGrammar.NLs &
          LexerGrammar.RPAREN)
      .map<List<dynamic>>((value) {
    final arguments = value[1] as List<dynamic>;
    return <dynamic>[
      value[0] as OperatorToken, // leftParenthesis
      arguments?.elementAt(0) as List<Argument>, // arguments
      arguments?.elementAt(1) as List<OperatorToken>, // argumentCommata
      value[3] as OperatorToken, // rightParenthesis
    ];
  });

  static final valueArguments = (LexerGrammar.NLs &
          valueArgument.fullCommaSeparatedList().optional() &
          LexerGrammar.NLs)
      .map((value) => value[1] as List<dynamic>);
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
    LexerGrammar.IntegerLiteral.map((l) => Literal<int>(_id++, l)),
    LexerGrammar.BooleanLiteral.map((l) => Literal<bool>(_id++, l)),
  ]);
  static final stringLiteral =
      (LexerGrammar.QUOTE & stringLiteralPart.star() & LexerGrammar.QUOTE)
          .map((value) => StringLiteral(
                _id++,
                leadingQuote: value[0] as OperatorToken,
                parts: value[1] as List<StringLiteralPart>,
                trailingQuote: value[2] as OperatorToken,
              ));
  static final stringLiteralPart =
      literalStringLiteralPart | interpolatedStringLiteralPart;
  static final literalStringLiteralPart = LexerGrammar.LiteralStringToken_.map(
      (value) => StringLiteralPart.literal(_id++, value));
  static final interpolatedStringLiteralPart = (LexerGrammar.LCURL &
          LexerGrammar.NLs &
          expression &
          LexerGrammar.NLs &
          LexerGrammar.RCURL)
      .map((value) => StringLiteralPart.interpolated(
            _id++,
            leadingBrace: value[0] as OperatorToken,
            expression: value[2] as Expression,
            trailingBrace: value[4] as OperatorToken,
          ));
  static final lambdaLiteral = (LexerGrammar.LCURL &
          (LexerGrammar.NLs &
                  valueParameter.fullCommaSeparatedList().optional() &
                  LexerGrammar.NLs &
                  LexerGrammar.EQUALS_GREATER)
              .optional() &
          LexerGrammar.NLs &
          lambdaExpressions &
          LexerGrammar.NLs &
          LexerGrammar.RCURL)
      .map<LambdaLiteral>((value) {
    final parameterSection = value[1] as List<dynamic>;
    final parameters = parameterSection?.elementAt(1) as List<dynamic>;
    return LambdaLiteral(
      _id++,
      leftBrace: value[0] as OperatorToken,
      valueParameters: parameters?.elementAt(0) as List<ValueParameter> ?? [],
      valueParameterCommata:
          parameters?.elementAt(1) as List<OperatorToken> ?? [],
      arrow: parameterSection?.elementAt(3) as OperatorToken,
      expressions: value[3] as List<Expression>,
      rightBrace: value[5] as OperatorToken,
    );
  });
  static final lambdaExpressions =
      ((expression & (semis & expression).map((v) => v[1] as Expression).star())
                  .map((v) => [v[0] as Expression, ...v[1] as List<Expression>])
                  .optional() &
              semis.optional())
          .map((values) => values[0] as List<Expression> ?? []);
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

  // SECTION: modifiers

  static final modifiers = modifier.plus();
  // ignore: unnecessary_cast
  static final modifier = (((LexerGrammar.PUBLIC as Parser<ModifierToken>) |
              LexerGrammar.MUT |
              LexerGrammar.STATIC |
              LexerGrammar.BUILTIN |
              LexerGrammar.EXTERNAL |
              LexerGrammar.OVERRIDE |
              LexerGrammar.CONST) &
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

  void complexGrouping<Left, T, Right>(
    Parser<Left> left,
    Parser<Right> right, {
    @required T Function(Left, T, Right) mapper,
  }) {
    group().wrapper<dynamic, T>(
      left,
      right,
      (dynamic left, value, dynamic right) =>
          mapper(left as Left, value, right as Right),
    );
  }

  void grouping<T>(
    Parser<OperatorToken> left,
    Parser<OperatorToken> right, {
    @required T Function(OperatorToken, T, OperatorToken) mapper,
  }) =>
      group().wrapper<OperatorToken, T>(left, right, mapper);

  void postfix<O, T>(Parser<O> operator, {@required T Function(T, O) mapper}) {
    group().postfix<O, T>(operator, mapper);
  }

  void postfixExpression(Parser<OperatorToken> operator) {
    postfix<OperatorToken, Expression>(
      operator,
      mapper: (operand, operator) => PostfixExpression(
        ParserGrammar._id++,
        operand: operand,
        operatorToken: operator,
      ),
    );
  }

  void prefix<O, T>(Parser<O> operator, {@required T Function(O, T) mapper}) {
    group().prefix<O, T>(operator, mapper);
  }

  void prefixExpression(Parser<OperatorToken> operator) {
    prefix<OperatorToken, Expression>(
      operator,
      mapper: (operator, operand) => PrefixExpression(
        ParserGrammar._id++,
        operatorToken: operator,
        operand: operand,
      ),
    );
  }

  void left<T>(
    Parser<OperatorToken> operator, {
    @required T Function(T, OperatorToken, T) mapper,
  }) =>
      group().left<List<dynamic>, T>(
        LexerGrammar.NLs & operator & LexerGrammar.NLs,
        (left, operator, right) =>
            mapper(left, operator[1] as OperatorToken, right),
      );
  void leftComplex<O, T>(
    Parser<O> operator, {
    @required T Function(T, O, T) mapper,
  }) =>
      group().left<O, T>(
        operator,
        (left, operator, right) => mapper(left, operator, right),
      );

  void leftExpression(Parser<OperatorToken> operator) {
    left<Expression>(
      operator,
      mapper: (left, operator, right) =>
          BinaryExpression(ParserGrammar._id++, left, operator, right),
    );
  }

  void right(Parser<OperatorToken> operator) {
    group().right<OperatorToken, Expression>(
      (LexerGrammar.NLs & operator & LexerGrammar.NLs)
          .map((value) => value[1] as OperatorToken),
      (left, operator, right) =>
          BinaryExpression(ParserGrammar._id++, left, operator, right),
    );
  }
}
