import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';
import 'package:supernova/supernova_io.dart';

import '../ast.dart';
import '../context.dart';
import '../error.dart';
import '../file.dart';
import '../lexer/token.dart';
import '../position.dart';

class Parser {
  Parser._(this.context, this.file, this.tokens);

  final Context context;
  final SourceFile file;
  final List<Token> tokens;

  static Ast parse(Context context, SourceFile file, List<Token> tokens) {
    final parser = Parser._(context, file, tokens);
    return parser._parse();
  }

  static const RBRACKET_COMMA_COLON_SET = SyntaxElementTypeSet.of({
    TokenKind_ClosingBracket,
    TokenKind_Comma,
    TokenKind_Colon,
  });

  static const TOP_LEVEL_DECLARATION_FIRST = SyntaxElementTypeSet.of({
    TokenKind_StructKeyword,
    TokenKind_EnumKeyword,
    TokenKind_TraitKeyword,
    TokenKind_ImplKeyword,
    TokenKind_FunKeyword,
    TokenKind_LetKeyword,
  });
  static final LBRACKET_EQ_TOP_LEVEL_DECLARATION_FIRST_SET =
      const SyntaxElementTypeSet.of({
        TokenKind_OpeningBracket,
        TokenKind_Equal,
      }) +
      TOP_LEVEL_DECLARATION_FIRST;

  static final CLASS_NAME_RECOVERY_SET =
      const SyntaxElementTypeSet.of({
        TokenKind_OpeningBracket,
        TokenKind_OpeningParenthesis,
        TokenKind_Colon,
        TokenKind_OpeningBrace,
      }) +
      TOP_LEVEL_DECLARATION_FIRST;

  static const TYPE_PARAMETER_RBRACKET_RECOVERY_SET = SyntaxElementTypeSet.of({
    TokenKind_WhereKeyword,
    TokenKind_OpeningParenthesis,
    TokenKind_Colon,
    TokenKind_OpeningBrace,
    TokenKind_ClosingBracket,
  });
  static const PARAMETER_NAME_RECOVERY_SET = SyntaxElementTypeSet.of({
    TokenKind_Colon,
    TokenKind_Equal,
    TokenKind_Comma,
    TokenKind_ClosingParenthesis,
    TokenKind_LetKeyword,
  });
  static const TYPE_REF_FIRST = SyntaxElementTypeSet.of({
    TokenKind_OpeningBracket,
    TokenKind_Identifier,
    TokenKind_OpeningParenthesis,
  });
  static final LBRACE_RBRACE_TYPE_REF_FIRST_SET =
      const SyntaxElementTypeSet.of({
        TokenKind_OpeningBrace,
        TokenKind_ClosingBrace,
      }) +
      TYPE_REF_FIRST;
  static final COLON_COMMA_LBRACE_RBRACE_TYPE_REF_FIRST_SET =
      const SyntaxElementTypeSet.of({
        TokenKind_Colon,
        TokenKind_Comma,
        TokenKind_OpeningBrace,
        TokenKind_ClosingBrace,
      }) +
      TYPE_REF_FIRST;

  // static final MODIFIER_WITHOUT_FUN = TokenKind_MODIFIERS - TokenKind_FunKeyword;
  // static final VALUE_PARAMETER_FIRST =
  //     SyntaxElementTypeSet.of({TokenKind_Identifier, TokenKind_OpeningBracket, TokenKind_LetKeyword}) +
  //             MODIFIER_WITHOUT_FUN;
  // static final LAMBDA_VALUE_PARAMETER_FIRST =
  //     SyntaxElementTypeSet.of({TokenKind_Identifier, TokenKind_OpeningBracket}) +
  //             MODIFIER_WITHOUT_FUN;

  static const COMMA_COLON_RBRACKET_SET = SyntaxElementTypeSet.of({
    TokenKind_Comma,
    TokenKind_Colon,
    TokenKind_ClosingBracket,
  });
  static const IDENTIFIER_RBRACKET_LBRACKET_SET = SyntaxElementTypeSet.of({
    TokenKind_Identifier,
    TokenKind_ClosingBracket,
    TokenKind_OpeningBracket,
  });
  static const LBRACE_RBRACE_SET = SyntaxElementTypeSet.of({
    TokenKind_OpeningBrace,
    TokenKind_ClosingBrace,
  });
  static const COMMA_RBRACE_SET = SyntaxElementTypeSet.of({
    TokenKind_Comma,
    TokenKind_ClosingBrace,
  });
  static const VALUE_ARGS_RECOVERY_SET = SyntaxElementTypeSet.of({
    TokenKind_OpeningBrace,
    TokenKind_ClosingParenthesis,
    TokenKind_LineBreak,
    TokenKind_ClosingBrace,
  });
  // static final PROPERTY_NAME_FOLLOW_MULTI_DECLARATION_RECOVERY_SET =
  //     PROPERTY_NAME_FOLLOW_SET + PARAMETER_NAME_RECOVERY_SET;
  // static final PROPERTY_NAME_FOLLOW_FUNCTION_OR_PROPERTY_RECOVERY_SET =
  //     PROPERTY_NAME_FOLLOW_SET + LBRACE_RBRACE_SET + TOP_LEVEL_DECLARATION_FIRST;
  static const IDENTIFIER_EQ_COLON_SET = SyntaxElementTypeSet.of({
    TokenKind_Identifier,
    TokenKind_Equal,
    TokenKind_Colon,
  });
  static const COMMA_RPAR_RBRACKET_COLON_EQ_SET = SyntaxElementTypeSet.of({
    TokenKind_Comma,
    TokenKind_ClosingParenthesis,
    TokenKind_ClosingBracket,
    TokenKind_Colon,
    TokenKind_Equal,
  });
  // static final ACCESSOR_FIRST_OR_PROPERTY_END =
  //     TokenKind_MODIFIERS +
  //             SyntaxElementTypeSet.of(
  //                 TokenKind_AT,
  //                 TokenKind_LineBreak,
  //                 TokenKind_ClosingBrace
  //             );
  static const RPAR_IDENTIFIER_COLON_LBRACE_EQ_SET = SyntaxElementTypeSet.of({
    TokenKind_ClosingParenthesis,
    TokenKind_Identifier,
    TokenKind_Colon,
    TokenKind_OpeningBrace,
    TokenKind_Equal,
  });
  static const COMMA_COLON_RPAR_SET = SyntaxElementTypeSet.of({
    TokenKind_Comma,
    TokenKind_Colon,
    TokenKind_ClosingParenthesis,
  });
  static const RPAR_COLON_LBRACE_EQ_SET = SyntaxElementTypeSet.of({
    TokenKind_ClosingParenthesis,
    TokenKind_Colon,
    TokenKind_OpeningBrace,
    TokenKind_Equal,
  });
  static const LBRACKET_LBRACE_RBRACE_LPAR_SET = SyntaxElementTypeSet.of({
    TokenKind_OpeningBracket,
    TokenKind_OpeningBrace,
    TokenKind_ClosingBrace,
    TokenKind_OpeningParenthesis,
  });
  static const FUNCTION_NAME_FOLLOW_SET = SyntaxElementTypeSet.of({
    TokenKind_OpeningBracket,
    TokenKind_OpeningParenthesis,
    TokenKind_ClosingParenthesis,
    TokenKind_Colon,
    TokenKind_Equal,
  });
  static final FUNCTION_NAME_RECOVERY_SET =
      const SyntaxElementTypeSet.of({
        TokenKind_OpeningBracket,
        TokenKind_OpeningParenthesis,
        TokenKind_ClosingParenthesis,
        TokenKind_Colon,
        TokenKind_Equal,
      }) +
      LBRACE_RBRACE_SET +
      TOP_LEVEL_DECLARATION_FIRST;
  static const VALUE_PARAMETERS_FOLLOW_SET = SyntaxElementTypeSet.of({
    TokenKind_Equal,
    TokenKind_OpeningBrace,
    TokenKind_ClosingBrace,
    TokenKind_ClosingParenthesis,
  });
  static final LPAR_VALUE_PARAMETERS_FOLLOW_SET =
      const SyntaxElementTypeSet.of({TokenKind_OpeningParenthesis}) +
      VALUE_PARAMETERS_FOLLOW_SET;
  static const LPAR_LBRACE_COLON_SET = SyntaxElementTypeSet.of({
    TokenKind_OpeningParenthesis,
    TokenKind_OpeningBrace,
    TokenKind_Colon,
  });
  static const EOL_OR_RBRACE_SET = SyntaxElementTypeSet.of({
    TokenKind_LineBreak,
    TokenKind_ClosingBrace,
  });
  // static final CLASS_INTERFACE_SET = SyntaxElementTypeSet.of({TokenKind_CLASSKeyword, TokenKind_INTERFACEKeyword});

  var _current = 0;

  bool at<K extends TokenKind>() => _peek().kind is K;
  bool atSet(SyntaxElementTypeSet set) => set.containsKind(_peek().kind);

  Token consume<K extends TokenKind>() {
    final token = _advanceIfKind<K>();
    if (token != null) return token;

    throw error('$K');
  }

  _ParseError error(String expected) {
    final token = _peek();
    context.addError(
      CompilerError(
        span: token.span,
        message: 'Found `${token.kind}`, expected $expected',
      ),
    );
    return const _ParseError();
  }

  Token? _advanceIfKind<K extends TokenKind>() =>
      _advanceIf((token) => token.kind is K);
  void advanceMany(int advanceTokenCount) {
    for (var i = 0; i < advanceTokenCount; i++) {
      advance(); // erroneous token
    }
  }

  Token advance() => _advanceIf((_) => true)!;
  Token? _advanceIf(Predicate<Token> predicate) {
    while (_current < tokens.length) {
      final current = tokens[_current];
      if (current.kind.isCommentOrWhitespace) {
        _current++;
        continue;
      }
      if (predicate(current)) {
        _current++;
        return current;
      }
      return null;
    }
    return null;
  }

  Token _peek() =>
      tokens
          .skip(_current)
          .firstWhereOrNull((it) => !it.kind.isCommentOrWhitespace) ??
      endOfFileToken;
  late final endOfFileToken = Token(
    span: SourceSpan(file, .single(tokens.last.span.span.end)),
    kind: const TokenKind.endOfFile(),
  );
  bool get _isAtEnd => _peek().kind is TokenKind_EndOfFile;

  Ast _parse() {}
  // AstDeclaration? _parseDeclaration() {
  //   return switch (_peek()?.kind) {
  //     TokenKind_StructKeyword() => _parseStructDeclaration(),
  //     _ => null,
  //   };
  // }

  // AstStruct _parseStruct() {
  //   consume<TokenKind_StructKeyword>();
  //   final name = consume<TokenKind_Identifier>();
  //   consume<TokenKind_OpeningBrace>();
  //   consume<TokenKind_ClosingBrace>();
  // }

  AstFunction? parseFunction() {
    _require(at<TokenKind_FunKeyword>());
    final funKeyword = advance(); // fun keyword

    // Recovery for the case of class A { fun| }
    if (at<TokenKind_ClosingBrace>()) {
      error('Function body expected');
      return null;
    }

    // builder.disableJoiningComplexTokens()

    final name = parseFunctionOrPropertyName(
      'function',
      FUNCTION_NAME_RECOVERY_SET,
    );

    // builder.restoreJoiningComplexTokensState()

    var typeParametersOccurred = false;
    AstTypeParameters? typeParameters;
    if (at<TokenKind_OpeningBracket>()) {
      typeParameters = parseTypeParameters(LPAR_VALUE_PARAMETERS_FOLLOW_SET);
      typeParametersOccurred = true;
    }

    AstValueParameters? valueParameters;
    if (at<TokenKind_OpeningParenthesis>()) {
      valueParameters = parseValueParameters(
        isFunctionTypeContents: false,
        typeRequired: false,
        recoverySet: VALUE_PARAMETERS_FOLLOW_SET,
      );
    } else {
      error("Expecting '('");
    }

    AstType? returnType;
    if (at<TokenKind_Colon>()) {
      advance(); // COLON

      returnType = parseTypeRef();
    }

    // parseTypeConstraintsGuarded(typeParametersOccurred)

    AstBody? body;
    if (at<TokenKind_Equal>() || at<TokenKind_OpeningBrace>()) {
      body = parseFunctionBody();
    }

    return AstFunction(
      displaySpan: name?.span ?? funKeyword.span,
      name: name,
      typeParameters: typeParameters,
      valueParameters: valueParameters,
      returnType: returnType,
      body: body,
    );
  }

  Token? parseFunctionOrPropertyName(
    String title,
    SyntaxElementTypeSet recoverySet,
  ) {
    if (expectWithRemap<TokenKind_Identifier>()) {
      return advance();
    }

    errorWithRecovery('Expecting $title name', recoverySet: recoverySet);
    return null;
  }

  AstTypeParameters? parseTypeParameters({SyntaxElementTypeSet? recoverySet}) {
    final list = <AstErrorOr<AstTypeParameter>>[];
    if (!at<TokenKind_OpeningBracket>()) return null;

    advance(); // LBRACKET

    while (true) {
      if (at<TokenKind_Comma>()) {
        errorAndAdvance('Expecting type parameter declaration');
      }
      parseTypeParameter();

      if (!at<TokenKind_Comma>()) break;
      advance(); // COMMA
      if (at<TokenKind_ClosingBracket>()) {
        break;
      }
    }

    expect<TokenKind_ClosingBracket>("Missing ']'", recoverySet: recoverySet);
    return AstTypeParameters(parameters: list);
  }

  AstErrorOr<AstTypeParameter>? parseTypeParameter() {
    if (atSetWithRemap(TYPE_PARAMETER_RBRACKET_RECOVERY_SET)) {
      error('Type parameter declaration expected');
      return null;
    }

    // parseModifierList(GT_COMMA_COLON_SET);

    final name = expectIdentifierWithRemap(
      'Type parameter name expected',
      recoverySet: .empty,
    );

    AstErrorOr<AstType>? upperBound;
    if (at<TokenKind_Colon>()) {
      advance(); // COLON
      upperBound = Option.from(parseTypeRef());
    }

    return name == null
        ? const None()
        : Some(AstTypeParameter(name: name, upperBound: upperBound));
  }

  bool expect<K extends TokenKind>(
    String message, {
    SyntaxElementTypeSet? recoverySet,
  }) {
    if (at<K>()) {
      advance(); // expectation
      return true;
    }

    // if (K == TokenKind_Identifier && "`" == builder.tokenText) {
    //     advance();
    // }

    errorWithRecovery(message, recoverySet: recoverySet);

    return false;
  }

  Token? expectIdentifierWithRemap(
    String message, {
    SyntaxElementTypeSet? recoverySet,
  }) {
    final token = _peek();
    if (expectWithRemap<TokenKind_Identifier>()) {
      return token;
    }

    errorWithRecovery(message, recoverySet: recoverySet);
    return null;
  }

  bool expectWithRemap<K extends TokenKind>() {
    if (atWithRemap<K>()) {
      advance(); // expectation
      return true;
    }

    // if (K == TokenKind_Identifier && "`" == builder.tokenText) {
    //     advance();
    // }

    return false;
  }

  void errorWithRecovery(
    String message, {
    required SyntaxElementTypeSet? recoverySet,
  }) {
    final tt = _peek().kind;
    if (recoverySet == null ||
        recoverySet.containsKind(tt) ||
        tt is TokenKind_OpeningBrace ||
        tt is TokenKind_ClosingBrace ||
        (recoverySet.contains<TokenKind_LineBreak>() &&
            (_isAtEnd || /* tt is TokenKind_Semicolon || */
                builder.newlineBeforeCurrentToken()))) {
      error(message);
    } else {
      errorAndAdvance(message);
    }
  }

  void errorAndAdvance(String message, {int advanceTokenCount = 1}) {
    final err = mark();
    advanceMany(advanceTokenCount);
    err.error(message);
  }

  /// Use with caution since it has a side effect and can change tokens in the
  /// stream (remap soft keywords and modifiers).
  bool atWithRemap<K extends TokenKind>() {
    if (at<K>()) return true;
    //     if (_peek()?.kind is TokenKind_Identifier) {
    //         if (expectation.toString() == builder.tokenText) {
    //             builder.remapCurrentToken(expectation)
    //             return true
    //         }
    //     }
    //     if (expectation === TokenKind_Identifier) {
    //         if (TokenKind_isSoftKeywordOrModifier(builder.tokenText)) {
    //             builder.remapCurrentToken(TokenKind_Identifier)
    //             return true
    //         }
    //     }
    return false;
  }

  bool atSetWithRemap(SyntaxElementTypeSet set) {
    if (atSet(set)) return true;
    //     if (_peek()?.kind is TokenKind_Identifier) {
    //         if (set.containsWithRemap(builder.tokenText)) {
    //             builder.remapCurrentToken(TokenKind_Identifier)
    //             return true
    //         }
    //     }
    return false;
  }

  AstErrorOr<T> _parseOrError<T>(T Function() parse) {
    try {
      return Some(parse());
    } on _ParseError {
      return const None();
    }
  }

  // Type

  AstType parseType() {
    if (at<TokenKind_OpeningParenthesis>()) {
      advance();

      final parameterTypes = <AstType>[];
      while (true) {
        if (at<TokenKind_ClosingParenthesis>()) {
          break;
        }
        if (at<TokenKind_Comma>()) {
          errorAndAdvance('Expecting type');
          continue;
        }
        parameterTypes.add(parseType());

        if (!at<TokenKind_Comma>()) break;
        advance(); // COMMA
      }

      expect<TokenKind_Arrow>('Expecting `->` after function parameter types');

      final returnType = _parseOrError(parseType);

      return AstType.function(
        parameterTypes: parameterTypes,
        returnType: returnType,
      );
    }

    return parseNamedType();
  }

  AstType_Named parseNamedType() {
    final name = consume<TokenKind_Identifier>();

    AstTypeArguments? typeArguments;
    if (at<TokenKind_OpeningBracket>()) {
      advance(); // LBRACKET
      typeArguments = parseTypeArguments();
    }

    return AstType_Named(name: name, typeArguments: typeArguments);
  }

  AstTypeArguments parseTypeArguments() {
    expect<TokenKind_OpeningBracket>("Expecting '[' to start type arguments");

    final arguments = <AstType>[];
    while (true) {
      if (at<TokenKind_ClosingBracket>()) {
        break;
      }
      if (at<TokenKind_Comma>()) {
        errorAndAdvance('Expecting type argument');
        continue;
      }
      arguments.add(parseType());

      if (!at<TokenKind_Comma>()) break;
      advance(); // COMMA
    }

    expect<TokenKind_ClosingBracket>("Expecting ']' after type arguments");

    return AstTypeArguments(arguments: arguments);
  }

  // Expression

  AstExpression parseExpression() {
    AstExpression expression;
    if (at<TokenKind_Identifier>()) {
      expression = .identifier(advance());
    } else if (at<TokenKind_Int>()) {
      final token = advance();
      expression = .int(
        value: (token.kind as TokenKind_Int).value,
        token: token,
      );
    } else if (at<TokenKind_OpeningQuote>()) {
      final parts = <AstTextPart>[];
      while (true) {
        if (at<TokenKind_ClosingQuote>()) {
          advance(); // closing quote
          break;
        } else if (at<TokenKind_RegularStringPart>()) {
          final token = advance();
          parts.add(.text(token: token, value: token.rawText));
        } else if (at<TokenKind_EscapedChar>()) {
          final token = advance();
          parts.add(
            .text(token: token, value: (token as TokenKind_EscapedChar).value),
          );
        } else {
          break;
        }
      }
      expression = .text(parts: parts);
    } else if (at<TokenKind_OpeningParenthesis>()) {
      advance(); // LPAR
      final innerExpression = parseExpression();
      expect<TokenKind_ClosingParenthesis>("Expecting ')' after expression");
      expression = .parenthesized(
        AstParenthesized(expression: innerExpression),
      );
    } else {
      throw error('Expression expected');
    }
  }
}

class _ParseError implements Exception {
  const _ParseError();
}

void _require(bool condition) {
  if (!condition) throw StateError('Condition not met');
}

class SyntaxElementTypeSet {
  const SyntaxElementTypeSet.of(this._inner);
  static const empty = SyntaxElementTypeSet.of({});

  final Set<Type> _inner;

  bool contains<K extends TokenKind>() => _inner.contains(K);
  bool containsKind(TokenKind kind) => _inner.contains(kind.runtimeType);

  SyntaxElementTypeSet operator +(SyntaxElementTypeSet other) =>
      SyntaxElementTypeSet.of({..._inner, ...other._inner});
}
