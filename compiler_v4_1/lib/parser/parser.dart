import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import '../ast.dart';
import '../context.dart';
import '../error.dart';
import '../file.dart';
import '../lexer/token.dart';
import '../position.dart';

class Parser {
  Parser._(this.context, this.file, this.tokens);

  static Ast parse(Context context, SourceFile file, List<Token> tokens) {
    final parser = Parser._(context, file, tokens);
    return parser._parse();
  }

  final Context context;
  final SourceFile file;
  final List<Token> tokens;

  // Token kinds that can start a top-level declaration. They are used for error
  // recovery: When something can't be parsed, we don't skip tokens that could
  // start the next declaration.
  static const _declarationFirstSet = SyntaxElementTypeSet.of({
    TokenKind_UseKeyword,
    TokenKind_PackageKeyword,
    TokenKind_PublicKeyword,
    TokenKind_StructKeyword,
    TokenKind_EnumKeyword,
    TokenKind_TraitKeyword,
    TokenKind_ImplKeyword,
    TokenKind_FunKeyword,
    TokenKind_LetKeyword,
    // Only starts a member of a trait or impl, but listing it here lets the
    // member loop recover on it just like on `fun`.
    TokenKind_TypeKeyword,
  });
  static const _typeFirstSet = SyntaxElementTypeSet.of({
    TokenKind_Identifier,
    TokenKind_OpeningParenthesis,
  });
  static final _declarationNameRecoverySet =
      const SyntaxElementTypeSet.of({
        TokenKind_OpeningBracket,
        TokenKind_OpeningParenthesis,
        TokenKind_Colon,
        TokenKind_Equal,
      }) +
      _declarationFirstSet;
  static final _typeRecoverySet =
      const SyntaxElementTypeSet.of({
        TokenKind_Comma,
        TokenKind_Colon,
        TokenKind_Equal,
        TokenKind_ClosingParenthesis,
        TokenKind_ClosingBracket,
      }) +
      _declarationFirstSet;

  var _current = 0;

  /// The number of non-trivia tokens that have been consumed so far.
  ///
  /// This is used to detect whether a parse method made any progress, which
  /// guarantees that our loops terminate.
  var _consumedTokenCount = 0;

  // Low-level token handling

  bool at<K extends TokenKind>() => _peek().kind is K;
  bool atSet(SyntaxElementTypeSet set) => set.containsKind(_peek().kind);
  bool get _isAtEnd => _peek().kind is TokenKind_EndOfFile;

  Token _peek() => tokens.elementAtOrNull(_nextTokenIndex) ?? endOfFileToken;

  /// The index of the next token that is neither a comment nor whitespace.
  int get _nextTokenIndex {
    var index = _current;
    while (index < tokens.length && tokens[index].kind.isCommentOrWhitespace) {
      index++;
    }
    return index;
  }

  late final endOfFileToken = Token(
    span: SourceSpan(
      file,
      .single(tokens.isEmpty ? const Offset(0) : tokens.last.span.span.end),
    ),
    kind: const .endOfFile(),
  );

  /// Consumes and returns the next token.
  ///
  /// At the end of the file, the end-of-file token is returned without
  /// consuming anything.
  Token advance() {
    final index = _nextTokenIndex;
    if (index >= tokens.length) return endOfFileToken;

    final token = tokens[index];
    if (token.kind is TokenKind_EndOfFile) return token;

    _current = index + 1;
    _consumedTokenCount++;
    return token;
  }

  Token? _advanceIfKind<K extends TokenKind>() => at<K>() ? advance() : null;

  // Errors

  void error(String message) => _errorAt(_peek().span, message);
  void _errorAt(SourceSpan span, String message) =>
      context.addError(CompilerError(span: span, message: message));

  void errorAndAdvance(String message) {
    error(message);
    advance();
  }

  /// Reports [message] and skips the offending token unless it might belong to
  /// the enclosing construct (i.e., unless it's in [recoverySet]).
  ///
  /// Delimiters are never skipped: Consuming them would make us continue
  /// parsing the enclosing construct as if it was still open, which causes an
  /// avalanche of follow-up errors.
  void errorWithRecovery(
    String message, {
    required SyntaxElementTypeSet? recoverySet,
  }) {
    final kind = _peek().kind;
    if (_isAtEnd ||
        recoverySet == null ||
        recoverySet.containsKind(kind) ||
        kind is TokenKind_OpeningBrace ||
        kind is TokenKind_ClosingBrace ||
        kind is TokenKind_ClosingParenthesis ||
        kind is TokenKind_ClosingBracket) {
      error(message);
    } else {
      errorAndAdvance(message);
    }
  }

  /// Consumes a token of kind [K], or reports [message] if there is none.
  Token? expect<K extends TokenKind>(
    String message, {
    SyntaxElementTypeSet? recoverySet,
  }) {
    final token = _advanceIfKind<K>();
    if (token != null) return token;

    errorWithRecovery(message, recoverySet: recoverySet);
    return null;
  }

  AstErrorOr<Token> expectIdentifier(
    String message, {
    SyntaxElementTypeSet? recoverySet,
  }) => Option.from(
    expect<TokenKind_Identifier>(message, recoverySet: recoverySet),
  );

  SourceSpan _spanFromTo(Token start, Token? end) => SourceSpan(
    file,
    start.span.span.start.rangeTo((end ?? start).span.span.end),
  );

  String _withArticle(String noun) =>
      'aeiou'.contains(noun[0]) ? 'an $noun' : 'a $noun';

  /// Parses a comma-separated list of items until [isAtEnd] holds.
  ///
  /// [itemDescription] is a noun phrase without an article, e.g., `struct
  /// field`. [parseItem] returns `null` if the current token can't start an
  /// item, in which case that token is skipped.
  List<T> _parseCommaSeparated<T>(
    String itemDescription,
    ValueGetter<bool> isAtEnd,
    ValueGetter<T?> parseItem,
  ) {
    final items = <T>[];
    while (!isAtEnd() && !_isAtEnd) {
      final consumedTokenCountBefore = _consumedTokenCount;

      final item = parseItem();
      if (item == null) {
        errorAndAdvance('Expected ${_withArticle(itemDescription)}.');
        continue;
      }
      items.add(item);

      if (_advanceIfKind<TokenKind_Comma>() == null && !isAtEnd()) {
        error('This $itemDescription is missing a comma.');
      }

      // Ensure that we always make progress, even if the item was parsed
      // entirely from errors.
      if (_consumedTokenCount == consumedTokenCountBefore) advance();
    }
    return items;
  }

  // Declarations

  Ast _parse() {
    final uses = <AstUse>[];
    while (at<TokenKind_UseKeyword>()) {
      uses.add(parseUse());
    }

    final declarations = <AstDeclaration>[];
    while (!_isAtEnd) {
      if (at<TokenKind_UseKeyword>()) {
        error('`use` must come before all declarations.');
        uses.add(parseUse());
        continue;
      }

      final visibility = _parseVisibility();
      final declaration = parseDeclaration(visibility);
      if (declaration == null) {
        errorAndAdvance('Expected a declaration.');
        continue;
      }

      declarations.add(declaration);
    }
    return Ast(uses: uses, declarations: declarations);
  }

  AstVisibility _parseVisibility() {
    if (_advanceIfKind<TokenKind_PackageKeyword>() != null) return .package;
    if (_advanceIfKind<TokenKind_PublicKeyword>() != null) return .public;
    return .private;
  }

  AstUse parseUse() {
    final useKeyword = advance();

    final path = <Token>[];
    if (_advanceIfKind<TokenKind_PackageKeyword>() case final root?) {
      path.add(root);
    } else {
      final name = expectIdentifier(
        'This `use` is missing a module path.',
        recoverySet: _declarationFirstSet,
      );
      if (name.toNullable() case final token?) path.add(token);
    }

    while (_advanceIfKind<TokenKind_Dot>() != null) {
      final segment = expectIdentifier(
        'This `use` path is missing a segment after the dot.',
        recoverySet: _declarationFirstSet,
      );
      if (segment.toNullable() case final token?) path.add(token);
    }

    return AstUse(useKeyword: useKeyword, path: path);
  }

  AstDeclaration? parseDeclaration(AstVisibility visibility) =>
      switch (_peek().kind) {
        TokenKind_StructKeyword() => .struct(parseStruct(visibility)),
        TokenKind_EnumKeyword() => .enum_(parseEnum(visibility)),
        TokenKind_TraitKeyword() => .trait(parseTrait(visibility)),
        TokenKind_ImplKeyword() => .impl(parseImpl(visibility)),
        TokenKind_LetKeyword() => .assignment(parseAssignment(visibility)),
        TokenKind_FunKeyword() => .function(parseFunction(visibility)),
        _ => null,
      };

  AstStruct parseStruct(AstVisibility visibility) {
    advance(); // `struct`

    final name = expectIdentifier(
      'This struct is missing a name.',
      recoverySet: _declarationNameRecoverySet,
    );

    final typeParameters = at<TokenKind_OpeningBracket>()
        ? parseTypeParameters()
        : null;

    final AstStructKind kind;
    if (_advanceIfKind<TokenKind_Equal>() != null) {
      expect<TokenKind_BuiltinKeyword>(
        'This struct is missing the builtin keyword.',
        recoverySet: _declarationFirstSet,
      );
      kind = const .builtin();
    } else {
      expect<TokenKind_OpeningBrace>(
        'This struct is missing an opening curly brace.',
        recoverySet: _declarationFirstSet,
      );

      final fields = _parseCommaSeparated(
        'struct field',
        () => at<TokenKind_ClosingBrace>(),
        parseStructField,
      );

      expect<TokenKind_ClosingBrace>(
        'This struct is missing a closing curly brace.',
        recoverySet: _declarationFirstSet,
      );
      kind = .userDefined(fields: fields);
    }

    return AstStruct(
      visibility: visibility,
      name: name,
      typeParameters: typeParameters,
      kind: kind,
    );
  }

  AstStructField? parseStructField() {
    if (!at<TokenKind_Identifier>() &&
        !at<TokenKind_PackageKeyword>() &&
        !at<TokenKind_PublicKeyword>()) {
      return null;
    }

    final visibility = _parseVisibility();
    final name = expectIdentifier('This struct field is missing a name.');
    expect<TokenKind_Colon>(
      'This struct field is missing a colon.',
      recoverySet: _typeFirstSet,
    );
    final type = parseTypeOrError('This struct field is missing a type.');

    return AstStructField(visibility: visibility, name: name, type: type);
  }

  AstEnum parseEnum(AstVisibility visibility) {
    advance(); // `enum`

    final name = expectIdentifier(
      'This enum is missing a name.',
      recoverySet: _declarationNameRecoverySet,
    );

    final typeParameters = at<TokenKind_OpeningBracket>()
        ? parseTypeParameters()
        : null;

    expect<TokenKind_OpeningBrace>(
      'This enum is missing an opening curly brace.',
      recoverySet: _declarationFirstSet,
    );

    final variants = _parseCommaSeparated(
      'enum variant',
      () => at<TokenKind_ClosingBrace>(),
      parseEnumVariant,
    );

    expect<TokenKind_ClosingBrace>(
      'This enum is missing a closing curly brace.',
      recoverySet: _declarationFirstSet,
    );

    return AstEnum(
      visibility: visibility,
      name: name,
      typeParameters: typeParameters,
      variants: variants,
    );
  }

  AstEnumVariant? parseEnumVariant() {
    if (!at<TokenKind_Identifier>()) return null;

    final name = expectIdentifier('This enum variant is missing a name.');

    AstErrorOr<AstType>? type;
    if (_advanceIfKind<TokenKind_Colon>() != null) {
      type = parseTypeOrError('This enum variant is missing a type.');
    }

    return AstEnumVariant(name: name, type: type);
  }

  AstTrait parseTrait(AstVisibility visibility) {
    advance(); // `trait`

    final name = expectIdentifier(
      'This trait is missing a name.',
      recoverySet: _declarationNameRecoverySet,
    );

    final typeParameters = at<TokenKind_OpeningBracket>()
        ? parseTypeParameters()
        : null;

    expect<TokenKind_OpeningBrace>(
      'This trait is missing an opening curly brace.',
      recoverySet: _declarationFirstSet,
    );

    final members = parseMembers(allowFunctionVisibility: false);

    expect<TokenKind_ClosingBrace>(
      'This trait is missing a closing curly brace.',
      recoverySet: _declarationFirstSet,
    );

    return AstTrait(
      visibility: visibility,
      name: name,
      typeParameters: typeParameters,
      associatedTypes: members.associatedTypes,
      functions: members.functions,
    );
  }

  AstImpl parseImpl(AstVisibility visibility) {
    if (visibility != .private) {
      error("An `impl` can't have a visibility modifier.");
    }

    final implKeyword = advance(); // `impl`

    final typeParameters = at<TokenKind_OpeningBracket>()
        ? parseTypeParameters()
        : null;

    final type = parseTypeOrError(
      'This impl is missing a type.',
      recoverySet: _typeRecoverySet,
    );

    final trait = _advanceIfKind<TokenKind_Colon>() == null
        ? null
        : parseTypeOrError(
            'This impl is missing a trait.',
            recoverySet: _typeRecoverySet,
          );

    expect<TokenKind_OpeningBrace>(
      'This impl is missing an opening curly brace.',
      recoverySet: _declarationFirstSet,
    );

    final members = parseMembers(allowFunctionVisibility: trait == null);

    expect<TokenKind_ClosingBrace>(
      'This impl is missing a closing curly brace.',
      recoverySet: _declarationFirstSet,
    );

    return AstImpl(
      implKeyword: implKeyword,
      typeParameters: typeParameters,
      type: type,
      trait: trait,
      associatedTypes: members.associatedTypes,
      functions: members.functions,
    );
  }

  ({List<AstAssociatedType> associatedTypes, List<AstFunction> functions})
  parseMembers({required bool allowFunctionVisibility}) {
    final associatedTypes = <AstAssociatedType>[];
    final functions = <AstFunction>[];
    while (!at<TokenKind_ClosingBrace>() && !_isAtEnd) {
      final visibility = _parseVisibility();

      if (at<TokenKind_TypeKeyword>()) {
        if (visibility != .private) {
          error("An associated type can't have a visibility modifier.");
        }
        associatedTypes.add(parseAssociatedType());
        continue;
      }
      if (at<TokenKind_FunKeyword>()) {
        if (!allowFunctionVisibility && visibility != .private) {
          error(
            "A function in a trait or a trait impl can't have a visibility "
            'modifier.',
          );
        }
        functions.add(
          parseFunction(allowFunctionVisibility ? visibility : .private),
        );
        continue;
      }

      if (visibility == .private && atSet(_declarationFirstSet)) break;

      errorAndAdvance('Expected a function or an associated type.');
    }
    return (associatedTypes: associatedTypes, functions: functions);
  }

  AstAssociatedType parseAssociatedType() {
    final typeKeyword = advance(); // `type`

    final name = expectIdentifier(
      'This associated type is missing a name.',
      recoverySet: _declarationNameRecoverySet,
    );

    AstErrorOr<AstType>? upperBound;
    if (_advanceIfKind<TokenKind_Colon>() != null) {
      upperBound = parseTypeOrError(
        'This associated type is missing an upper bound after the colon.',
        recoverySet: _typeRecoverySet,
      );
    }

    AstErrorOr<AstType>? value;
    if (_advanceIfKind<TokenKind_Equal>() != null) {
      value = parseTypeOrError(
        'This associated type is missing a type after the equals sign.',
        recoverySet: _typeRecoverySet,
      );
    }

    return AstAssociatedType(
      displaySpan: name.toNullable()?.span ?? typeKeyword.span,
      name: name,
      upperBound: upperBound,
      value: value,
    );
  }

  AstAssignment parseAssignment(AstVisibility visibility) {
    final letKeyword = advance(); // `let`

    final name = expectIdentifier(
      'This assignment is missing a name.',
      recoverySet: _declarationNameRecoverySet,
    );

    AstErrorOr<AstType>? type;
    if (_advanceIfKind<TokenKind_Colon>() != null) {
      type = parseTypeOrError(
        'This assignment is missing a type after the colon.',
        recoverySet: _typeRecoverySet,
      );
    }

    expect<TokenKind_Equal>('This assignment is missing an equals sign.');

    final value = parseExpressionOrError(
      'This assignment is missing a value.',
      recoverySet: _declarationFirstSet,
    );

    return AstAssignment(
      visibility: visibility,
      displaySpan: name.toNullable()?.span ?? letKeyword.span,
      name: name,
      type: type,
      value: value,
    );
  }

  AstFunction parseFunction(AstVisibility visibility) {
    final funKeyword = advance(); // `fun`

    final name = expectIdentifier(
      'This function is missing a name.',
      recoverySet: _declarationNameRecoverySet,
    );

    final typeParameters = at<TokenKind_OpeningBracket>()
        ? parseTypeParameters()
        : null;

    final AstErrorOr<AstValueParameters> valueParameters;
    if (at<TokenKind_OpeningParenthesis>()) {
      valueParameters = Some(parseValueParameters());
    } else {
      errorWithRecovery(
        'This function is missing parameters.',
        recoverySet: _declarationFirstSet,
      );
      valueParameters = const None();
    }

    // Unlike the parameter types, the return type isn't preceded by anything,
    // so we only try to parse it if a type can start here.
    final returnType = atSet(_typeFirstSet) ? parseType() : null;

    final body = at<TokenKind_OpeningBrace>() ? parseBody() : null;

    return AstFunction(
      visibility: visibility,
      displaySpan: name.toNullable()?.span ?? funKeyword.span,
      name: name,
      typeParameters: typeParameters,
      valueParameters: valueParameters,
      returnType: returnType,
      body: body,
    );
  }

  AstValueParameters parseValueParameters({bool typeIsOptional = false}) {
    advance(); // `(`

    // TODO(JonasWanke): error on duplicate parameter names
    final parameters = _parseCommaSeparated(
      'parameter',
      () => at<TokenKind_ClosingParenthesis>(),
      () => parseValueParameter(typeIsOptional: typeIsOptional),
    );

    expect<TokenKind_ClosingParenthesis>(
      'This parameter list is missing a closing parenthesis.',
      recoverySet: _declarationFirstSet,
    );

    return AstValueParameters(valueParameters: parameters);
  }

  AstValueParameter? parseValueParameter({bool typeIsOptional = false}) {
    if (!at<TokenKind_Identifier>()) return null;

    final name = expectIdentifier('This parameter is missing a name.');
    if (typeIsOptional && !at<TokenKind_Colon>()) {
      return AstValueParameter(name: name, type: null);
    }

    expect<TokenKind_Colon>(
      'This parameter is missing a colon.',
      recoverySet: _typeFirstSet,
    );
    final type = parseTypeOrError(
      'This parameter is missing a type.',
      recoverySet: _typeRecoverySet,
    );

    return AstValueParameter(name: name, type: type);
  }

  AstTypeParameters parseTypeParameters() {
    final openingBracket = advance(); // `[`

    // TODO(JonasWanke): error on duplicate type parameter names
    final parameters = _parseCommaSeparated(
      'type parameter',
      () => at<TokenKind_ClosingBracket>(),
      parseTypeParameter,
    );

    final closingBracket = expect<TokenKind_ClosingBracket>(
      'These type parameters are missing a closing bracket.',
      recoverySet: _declarationFirstSet,
    );

    if (parameters.isEmpty) {
      _errorAt(
        _spanFromTo(openingBracket, closingBracket),
        'Type parameter brackets must not be empty.',
      );
    }

    return AstTypeParameters(parameters: parameters);
  }

  AstTypeParameter? parseTypeParameter() {
    if (!at<TokenKind_Identifier>()) return null;

    final name = expectIdentifier('This type parameter is missing a name.');

    AstErrorOr<AstType>? upperBound;
    if (_advanceIfKind<TokenKind_Colon>() != null) {
      upperBound = parseTypeOrError(
        'This type parameter is missing an upper bound after the colon.',
        recoverySet: _typeRecoverySet,
      );
    }

    return AstTypeParameter(name: name, upperBound: upperBound);
  }

  // Types

  AstErrorOr<AstType> parseTypeOrError(
    String message, {
    SyntaxElementTypeSet? recoverySet,
  }) {
    final type = parseType();
    if (type != null) return Some(type);

    errorWithRecovery(message, recoverySet: recoverySet);
    return const None();
  }

  AstType? parseType() {
    final AstType base;
    if (at<TokenKind_Identifier>()) {
      base = parseNamedType();
    } else if (at<TokenKind_OpeningParenthesis>()) {
      base = parseFunctionType();
    } else {
      return null;
    }

    var type = base;
    while (_advanceIfKind<TokenKind_Dot>() != null) {
      final name = expectIdentifier(
        'This associated type is missing a name after the dot.',
        recoverySet: _typeRecoverySet,
      ).toNullable();
      if (name == null) break;

      type = AstType.associated(baseType: type, name: name);
    }
    return type;
  }

  AstType_Named parseNamedType() {
    final name = advance(); // identifier

    final typeArguments = at<TokenKind_OpeningBracket>()
        ? parseTypeArguments()
        : null;

    return AstType_Named(name: name, typeArguments: typeArguments);
  }

  AstType_Function parseFunctionType() {
    advance(); // `(`

    final parameterTypes = _parseCommaSeparated(
      'parameter type',
      () => at<TokenKind_ClosingParenthesis>(),
      parseType,
    );

    expect<TokenKind_ClosingParenthesis>(
      'These parameter types are missing a closing parenthesis.',
      recoverySet: _typeFirstSet,
    );

    final returnType = parseTypeOrError(
      'This function type is missing a return type.',
      recoverySet: _typeRecoverySet,
    );

    return AstType_Function(
      parameterTypes: parameterTypes,
      returnType: returnType,
    );
  }

  AstTypeArguments parseTypeArguments() {
    final openingBracket = advance(); // `[`

    final arguments = _parseCommaSeparated(
      'type argument',
      () => at<TokenKind_ClosingBracket>(),
      parseType,
    );

    final closingBracket = expect<TokenKind_ClosingBracket>(
      'These type arguments are missing a closing bracket.',
      recoverySet: _typeRecoverySet,
    );

    if (arguments.isEmpty) {
      _errorAt(
        _spanFromTo(openingBracket, closingBracket),
        'Type argument brackets must not be empty.',
      );
    }

    return AstTypeArguments(arguments: arguments);
  }

  // Expressions

  AstErrorOr<AstExpression> parseExpressionOrError(
    String message, {
    SyntaxElementTypeSet? recoverySet,
  }) {
    final expression = parseExpression();
    if (expression != null) return Some(expression);

    errorWithRecovery(message, recoverySet: recoverySet);
    return const None();
  }

  AstExpression? parseExpression() {
    final primary = parsePrimaryExpression();
    if (primary == null) return null;

    var expression = primary;
    while (true) {
      if (at<TokenKind_Dot>()) {
        expression = .navigation(parseNavigation(expression));
        continue;
      }
      if (at<TokenKind_OpeningBracket>() ||
          at<TokenKind_OpeningParenthesis>()) {
        expression = .call(parseCall(expression));
        continue;
      }
      break;
    }
    return expression;
  }

  AstExpression? parsePrimaryExpression() {
    if (at<TokenKind_Identifier>()) return AstExpression.identifier(advance());
    if (at<TokenKind_Int>()) {
      final token = advance();
      return AstExpression.int(
        value: (token.kind as TokenKind_Int).value,
        token: token,
      );
    }
    if (at<TokenKind_OpeningQuote>()) return parseText();
    if (at<TokenKind_SwitchKeyword>()) {
      return AstExpression.switch_(parseSwitch());
    }
    if (at<TokenKind_OpeningBrace>()) return AstExpression.body(parseBody());
    if (at<TokenKind_OpeningParenthesis>()) {
      return _isAtLambda()
          ? .lambda(parseLambda())
          : .parenthesized(parseParenthesized());
    }
    return null;
  }

  /// Whether the parenthesis at the current position starts a lambda's
  /// parameter list (rather than a parenthesized expression).
  ///
  /// Both start with `(`, but only a lambda has a body after the matching `)`.
  bool _isAtLambda() {
    var index = _nextTokenIndex;
    var depth = 0;
    for (; index < tokens.length; index++) {
      final kind = tokens[index].kind;
      if (kind is TokenKind_OpeningParenthesis) {
        depth++;
      } else if (kind is TokenKind_ClosingParenthesis) {
        depth--;
        if (depth == 0) {
          index++;
          break;
        }
      }
    }

    for (; index < tokens.length; index++) {
      final kind = tokens[index].kind;
      if (kind.isCommentOrWhitespace) continue;

      return kind is TokenKind_OpeningBrace;
    }
    return false;
  }

  AstExpression_Text parseText() {
    advance(); // `"`

    final parts = <AstTextPart>[];
    while (true) {
      if (_advanceIfKind<TokenKind_ClosingQuote>() != null) break;

      if (at<TokenKind_RegularStringPart>()) {
        final token = advance();
        parts.add(.text(token: token, value: token.rawText));
      } else if (at<TokenKind_EscapedChar>()) {
        final token = advance();
        parts.add(
          .text(
            token: token,
            value: (token.kind as TokenKind_EscapedChar).value,
          ),
        );
      } else if (at<TokenKind_TextInterpolationStart>()) {
        parts.add(parseTextInterpolation());
      } else {
        // The lexer already reported that the text isn't terminated.
        break;
      }
    }

    return AstExpression_Text(parts: parts);
  }

  AstTextPart parseTextInterpolation() {
    advance(); // `{`

    final expression = parseExpressionOrError(
      "Here's a start of a text interpolation without an expression after it.",
    );

    expect<TokenKind_TextInterpolationEnd>(
      "This text interpolation isn't closed.",
    );

    return AstTextPart.interpolation(expression: expression);
  }

  AstParenthesized parseParenthesized() {
    advance(); // `(`

    final inner = parseExpressionOrError(
      'This parenthesized expression is missing a value.',
    );

    expect<TokenKind_ClosingParenthesis>(
      'This parenthesized expression is missing a closing parenthesis.',
    );

    return AstParenthesized(inner: inner);
  }

  AstLambda parseLambda() {
    final parameters = parseValueParameters(typeIsOptional: true);
    final body = parseBody();
    return AstLambda(parameters: parameters, body: body);
  }

  AstNavigation parseNavigation(AstExpression receiver) {
    advance(); // `.`

    final key = expectIdentifier('This struct access is missing a key.');

    return AstNavigation(receiver: receiver, key: key);
  }

  AstCall parseCall(AstExpression receiver) {
    final typeArguments = at<TokenKind_OpeningBracket>()
        ? parseTypeArguments()
        : null;

    final AstErrorOr<AstArguments> arguments;
    if (at<TokenKind_OpeningParenthesis>()) {
      arguments = Some(parseArguments());
    } else {
      error('This call is missing arguments.');
      arguments = const None();
    }

    return AstCall(
      receiver: receiver,
      typeArguments: typeArguments,
      arguments: arguments,
    );
  }

  AstArguments parseArguments() {
    final openingParenthesis = advance(); // `(`

    final arguments = _parseCommaSeparated(
      'argument',
      () => at<TokenKind_ClosingParenthesis>(),
      parseExpression,
    );

    expect<TokenKind_ClosingParenthesis>(
      'This call is missing a closing parenthesis.',
    );

    return AstArguments(
      openingParenthesisSpan: openingParenthesis.span,
      arguments: arguments,
    );
  }

  AstBody parseBody() {
    expect<TokenKind_OpeningBrace>(
      'This body is missing an opening curly brace.',
    );

    final statements = <AstStatement>[];
    while (!at<TokenKind_ClosingBrace>() && !_isAtEnd) {
      final consumedTokenCountBefore = _consumedTokenCount;

      final statement = parseStatement();
      if (statement == null) {
        errorAndAdvance('Expected a statement.');
        continue;
      }
      statements.add(statement);

      if (_consumedTokenCount == consumedTokenCountBefore) advance();
    }

    expect<TokenKind_ClosingBrace>(
      'This body is missing a closing curly brace.',
    );

    return AstBody(statements: statements);
  }

  AstStatement? parseStatement() {
    if (at<TokenKind_LetKeyword>()) {
      return AstStatement.assignment(parseAssignment(.private));
    }

    final expression = parseExpression();
    return expression == null ? null : .expression(expression);
  }

  AstSwitch parseSwitch() {
    advance(); // `switch`

    final value = parseExpressionOrError(
      'This switch is missing an expression to switch over.',
    );

    expect<TokenKind_OpeningBrace>(
      'This switch is missing an opening curly brace.',
    );

    final cases = _parseCommaSeparated(
      'switch case',
      () => at<TokenKind_ClosingBrace>(),
      parseSwitchCase,
    );

    expect<TokenKind_ClosingBrace>(
      'This switch is missing a closing curly brace.',
    );

    return AstSwitch(value: value, cases: cases);
  }

  AstSwitchCase? parseSwitchCase() {
    if (!at<TokenKind_Identifier>()) return null;

    final variant = expectIdentifier('This switch case is missing a variant.');

    AstErrorOr<Token>? valueName;
    if (_advanceIfKind<TokenKind_OpeningParenthesis>() != null) {
      valueName = expectIdentifier('Switch case is missing a value name.');
      expect<TokenKind_ClosingParenthesis>(
        'Switch case is missing a closing parenthesis after the value name.',
      );
    }

    expect<TokenKind_Arrow>('Switch case is missing an arrow.');

    final expression = parseExpressionOrError(
      'This switch case is missing an expression.',
    );

    return AstSwitchCase(
      variant: variant,
      valueName: valueName,
      expression: expression,
    );
  }
}

/// A set of token kinds, used for error recovery.
class SyntaxElementTypeSet {
  const SyntaxElementTypeSet.of(this._inner);
  static const empty = SyntaxElementTypeSet.of({});

  final Set<Type> _inner;

  bool contains<K extends TokenKind>() => _inner.contains(K);
  bool containsKind(TokenKind kind) => _inner.contains(kind.runtimeType);

  SyntaxElementTypeSet operator +(SyntaxElementTypeSet other) =>
      SyntaxElementTypeSet.of({..._inner, ...other._inner});
}
