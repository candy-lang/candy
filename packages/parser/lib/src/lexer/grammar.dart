import 'package:meta/meta.dart';
import 'package:characters/characters.dart';
import 'package:petitparser/petitparser.dart'
    hide ChoiceParserExtension, Token, SequenceParserExtension;

import '../source_span.dart';
import '../utils.dart';
import 'token.dart';

// ignore_for_file: non_constant_identifier_names

// ignore: avoid_classes_with_only_static_members
@immutable
class LexerGrammar {
  static void init() {
    if (_isInitialized) return;
    _isInitialized = true;

    _initDelimitedComment();
  }

  static bool _isInitialized = false;

  // SECTION: lexicalGeneral

  static final _DelimitedComment = undefined<void>();
  static Parser<void> get DelimitedComment => _DelimitedComment;
  static void _initDelimitedComment() {
    _DelimitedComment.set(string('/*') &
        (DelimitedComment | any()).starLazy(string('*/')).flatten() &
        string('*/'));
  }

  static final Parser<void> LineComment = string('//') &
      NL.neg().star().ignore() &
      (NL | endOfInput()).and().ignore();

  static final Parser<void> WS = (char(' ') |
          char('\t') |
          DelimitedComment.ignore() |
          LineComment.ignore())
      .plus()
      .ignore();
  static final Parser<void> NL =
      (char('\n') | char('\r') & char('\n').optional()).ignore();
  static final Parser<void> NLs =
      (NL | WS | DelimitedComment | LineComment).star();

  // SECTION: separatorsAndOperations

  static final Parser<void> SEMICOLON = char(';').ignore();

  static final DOT = _operator('.', OperatorTokenType.dot);
  static final COMMA = _operator(',', OperatorTokenType.comma);
  static final COLON = _operator(':', OperatorTokenType.colon);
  static final HASHTAG = _operator('#', OperatorTokenType.hashtag);
  static final QUOTE = _operator('"', OperatorTokenType.quote);
  static final EQUALS_GREATER =
      _operator('=>', OperatorTokenType.equalsGreater);

  static final LPAREN = _operator('(', OperatorTokenType.lparen);
  static final RPAREN = _operator(')', OperatorTokenType.rparen);
  static final LSQUARE = _operator('[', OperatorTokenType.lsquare);
  static final RSQUARE = _operator(']', OperatorTokenType.rsquare);
  static final LCURL = _operator('{', OperatorTokenType.lcurl);
  static final RCURL = _operator('}', OperatorTokenType.rcurl);
  static final LANGLE = _operator('<', OperatorTokenType.langle);
  static final RANGLE = _operator('>', OperatorTokenType.rangle);

  static final PLUS_PLUS = _operator('++', OperatorTokenType.plusPlus);
  static final MINUS_MINUS = _operator('--', OperatorTokenType.minusMinus);
  static final QUESTION = _operator('?', OperatorTokenType.question);
  static final EXCLAMATION = _operator('!', OperatorTokenType.exclamation);

  static final TILDE = _operator('~', OperatorTokenType.tilde);

  static final ASTERISK = _operator('*', OperatorTokenType.asterisk);
  static final SLASH = _operator('/', OperatorTokenType.slash);
  static final TILDE_SLASH = _operator('~/', OperatorTokenType.tildeSlash);
  static final PERCENT = _operator('%', OperatorTokenType.percent);

  static final PLUS = _operator('+', OperatorTokenType.plus);
  static final MINUS = _operator('-', OperatorTokenType.minus);

  static final LESS_LESS = _operator('<<', OperatorTokenType.lessLess);
  static final GREATER_GREATER =
      _operator('>>', OperatorTokenType.greaterGreater);
  static final GREATER_GREATER_GREATER =
      _operator('>>>', OperatorTokenType.greaterGreaterGreater);

  static final AMPERSAND = _operator('&', OperatorTokenType.ampersand);

  static final CARET = _operator('^', OperatorTokenType.caret);

  static final BAR = _operator('|', OperatorTokenType.bar);

  static final AS = _operator('as', OperatorTokenType.as);
  static final AS_SAFE = _operator('as?', OperatorTokenType.asSafe);

  static final DOT_DOT = _operator('..', OperatorTokenType.dotDot);
  static final DOT_DOT_EQUALS =
      _operator('..=', OperatorTokenType.dotDotEquals);

  static final IN = _operator('in', OperatorTokenType.in_);
  static final EXCLAMATION_IN =
      _operator('!in', OperatorTokenType.exclamationIn);
  static final IS = _operator('is', OperatorTokenType.is_);
  static final EXCLAMATION_IS =
      _operator('!is', OperatorTokenType.exclamationIs);

  static final LESS = _operator('<', OperatorTokenType.less);
  static final LESS_EQUAL = _operator('<=', OperatorTokenType.lessEquals);
  static final GREATER = _operator('>', OperatorTokenType.greater);
  static final GREATER_EQUAL = _operator('>=', OperatorTokenType.greaterEquals);

  static final EQUALS_EQUALS = _operator('==', OperatorTokenType.equalsEquals);
  static final EXCLAMATION_EQUALS =
      _operator('!=', OperatorTokenType.exclamationEquals);
  static final EQUALS_EQUALS_EQUALS =
      _operator('===', OperatorTokenType.equalsEqualsEquals);
  static final EXCLAMATION_EQUALS_EQUALS =
      _operator('!==', OperatorTokenType.exclamationEqualsEquals);

  static final AMPERSAND_AMPERSAND =
      _operator('&&', OperatorTokenType.ampersandAmpersand);

  static final BAR_BAR = _operator('||', OperatorTokenType.barBar);

  static final DASH_GREATER = _operator('->', OperatorTokenType.dashGreater);
  static final LESS_DASH = _operator('<-', OperatorTokenType.lessDash);

  static final DOT_DOT_DOT = _operator('...', OperatorTokenType.dotDotDot);

  static final EQUALS = _operator('=', OperatorTokenType.equals);
  static final ASTERISK_EQUALS =
      _operator('*=', OperatorTokenType.asteriskEquals);
  static final SLASH_EQUALS = _operator('/=', OperatorTokenType.slashEquals);
  static final TILDE_SLASH_EQUALS =
      _operator('~/=', OperatorTokenType.tildeSlashEquals);
  static final PERCENT_EQUALS =
      _operator('%=', OperatorTokenType.percentEquals);
  static final PLUS_EQUALS = _operator('+=', OperatorTokenType.plusEquals);
  static final MINUS_EQUALS = _operator('-=', OperatorTokenType.minusEquals);
  static final AMPERSAND_EQUALS =
      _operator('&=', OperatorTokenType.ampersandEquals);
  static final BAR_EQUALS = _operator('|=', OperatorTokenType.barEquals);
  static final CARET_EQUALS = _operator('^=', OperatorTokenType.caretEquals);
  static final AMPERSAND_AMPERSAND_EQUALS =
      _operator('&&=', OperatorTokenType.ampersandAmpersandEquals);
  static final BAR_BAR_EQUALS =
      _operator('||=', OperatorTokenType.barBarEquals);
  static final LESS_LESS_EQUALS =
      _operator('<<=', OperatorTokenType.lessLessEquals);
  static final GREATER_GREATER_EQUALS =
      _operator('>>=', OperatorTokenType.greaterGreaterEquals);
  static final GREATER_GREATER_GREATER_EQUALS =
      _operator('>>>=', OperatorTokenType.greaterGreaterGreaterEquals);
  static Parser<OperatorToken> _operator(
          String operator, OperatorTokenType type) =>
      string(operator)
          .tokenize((lexeme, span) => OperatorToken(type, span: span));

  // SECTION: keywords

  static final USE =
      string('use').tokenize((_, span) => UseKeywordToken(span: span));
  static final CRATE =
      string('crate').tokenize((_, span) => CrateKeywordToken(span: span));
  static final MODULE =
      string('module').tokenize((_, span) => ModuleKeywordToken(span: span));
  static final TRAIT =
      string('trait').tokenize((_, span) => TraitKeywordToken(span: span));
  static final IMPL =
      string('impl').tokenize((_, span) => ImplKeywordToken(span: span));
  static final CLASS =
      string('class').tokenize((_, span) => ClassKeywordToken(span: span));
  static final FUN =
      string('fun').tokenize((_, span) => FunKeywordToken(span: span));
  static final LET =
      string('let').tokenize((_, span) => LetKeywordToken(span: span));
  static final GET =
      string('get').tokenize((_, span) => GetKeywordToken(span: span));
  static final SET =
      string('set').tokenize((_, span) => SetKeywordToken(span: span));

  static final IF = string('if')
      .tokenize((_, span) => KeywordToken.if_(span: span) as IfKeywordToken);
  static final ELSE = string('else').tokenize(
      (_, span) => KeywordToken.else_(span: span) as ElseKeywordToken);
  static final RETURN =
      string('return').tokenize((_, span) => ReturnKeywordToken(span: span));

  // SECTION: lexicalModifiers

  static final PUBLIC =
      string('public').tokenize((_, span) => PublicModifierToken(span: span));
  static final MUT =
      string('mut').tokenize((_, span) => MutModifierToken(span: span));
  static final STATIC =
      string('static').tokenize((_, span) => StaticModifierToken(span: span));
  static final BUILTIN =
      string('builtin').tokenize((_, span) => BuiltinModifierToken(span: span));
  static final EXTERNAL = string('external')
      .tokenize((_, span) => ExternalModifierToken(span: span));
  static final OVERRIDE = string('override')
      .tokenize((_, span) => OverrideModifierToken(span: span));
  static final CONST =
      string('const').tokenize((_, span) => ModifierToken.const_(span: span));

  // SECTION: literals

  static final IntegerLiteral = HexLiteral | BinLiteral | DecLiteral;

  // TODO(JonasWanke): disallow trailing underscore
  // formerly: DecDigit & DecDigitOrSeparator.star() & DecDigit | DecDigit
  static final DecLiteral =
      (_DecDigit & _DecDigitOrSeparator.star()).tokenizeInteger();

  // TODO(JonasWanke): disallow leading/trailing underscore
  // formerly: DecDigit & DecDigitOrSeparator.star() & DecDigit | DecDigit
  static final _DecDigits = _DecDigit & _DecDigitOrSeparator.star();

  static final _DecDigit = digit();
  static final _DecDigitOrSeparator = _DecDigit | char('_');

  // TODO: RealLiteral, FloatLiteral, DoubleLiteral

  // TODO(JonasWanke): disallow leading/trailing underscore
  // formerly: _hexLiteralPrefix & _HexDigit & (_HexDigitOrSeparator.star() & _HexDigit).optional()
  static final HexLiteral = (_hexLiteralPrefix & _HexDigitOrSeparator.plus())
      .tokenizeInteger(radix: 16, hasPrefix: true);
  static final _hexLiteralPrefix = char('0') & (char('x') | char('X'));

  static final _HexDigit = _DecDigit | _hexLettersLower | _hexLettersUpper;
  static final _hexLettersLower =
      char('a') | char('b') | char('c') | char('d') | char('e') | char('f');
  static final _hexLettersUpper =
      char('A') | char('B') | char('C') | char('D') | char('E') | char('F');
  static final _HexDigitOrSeparator = _HexDigit | char('_');

  // TODO(JonasWanke): disallow leading/trailing underscore
  // formerly: _binLiteralPrefix & _BinDigit & (_BinDigitOrSeparator.star() & _BinDigit).optional()
  static final BinLiteral = (_binLiteralPrefix & _BinDigitOrSeparator.plus())
      .tokenizeInteger(radix: 2, hasPrefix: true);
  static final _binLiteralPrefix = char('0') & (char('b') | char('B'));

  static final _BinDigit = char('0') | char('1');
  static final _BinDigitOrSeparator = _BinDigit | char('_');

  static final BooleanLiteral = (string('true') | string('false')).tokenize(
      (lexeme, span) => BoolLiteralToken(lexeme == 'true', span: span));

  static final LiteralStringToken_ =
      (QUOTE | LCURL).neg().plus().tokenize<LiteralStringToken>((lexeme, span) {
    final content = StringBuffer();
    var isEscaped = false;
    for (final char in lexeme.characters) {
      if (!isEscaped) {
        if (char == r'\') {
          isEscaped = true;
          continue;
        }
        content.write(char);
        continue;
      }

      final code = {
        'n': '\n',
        'r': '\r',
        't': '\t',
        r'\': '\\',
      }[char];
      if (code == null) {
        throw Exception('Invalid escaped character: `$char`.');
      }

      content.write(code);
      isEscaped = false;
    }

    return LiteralStringToken(content.toString(), span: span);
  });

  // SECTION: lexicalIdentifiers

  static final Identifier =
      ((Letter | char('_')) & (Letter | char('_') | _DecDigit).star())
          .tokenize((lexeme, span) => IdentifierToken(lexeme, span: span));

  // SECTION: characters

  static final Letter = letter();
}

typedef LiteralMapper<T extends Token> = T Function(
  String lexeme,
  SourceSpan span,
);

extension on Parser<void> {
  Parser<void> operator &(Parser<void> other) => SequenceParser([this, other]);
}

extension on Parser<String> {
  Parser<T> tokenize<T extends Token>(LiteralMapper<T> mapper) {
    return token().map((petitToken) {
      final span = SourceSpan(petitToken.start, petitToken.stop);
      return mapper(petitToken.value, span);
    });
  }

  Parser<OperatorToken> tokenizeOperator<T extends Token>(
    OperatorTokenType type,
  ) =>
      tokenize((lexeme, span) => OperatorToken(type, span: span));
  Parser<IntLiteralToken> tokenizeInteger({
    int radix = 10,
    bool hasPrefix = false,
  }) {
    return tokenize<IntLiteralToken>((lexeme, span) {
      return IntLiteralToken(
        int.parse(
          lexeme.substring(hasPrefix ? 2 : 0).replaceAll('_', ''),
          radix: radix,
        ),
        span: span,
      );
    });
  }

  Parser<String> operator &(Parser<String> other) =>
      SequenceParser([this, other]).flatten();

  Parser<String> star() =>
      PossessiveRepeatingParser(this, 0, unbounded).map((list) => list.join());
  Parser<String> plus() =>
      PossessiveRepeatingParser(this, 1, unbounded).map((list) => list.join());
}
