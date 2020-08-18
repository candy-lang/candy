import 'package:meta/meta.dart';
import 'package:petitparser/petitparser.dart'
    hide ChoiceParserExtension, Token, SequenceParserExtension;

import '../source_span.dart';
import '../utils.dart';
import 'token.dart';

// ignore_for_file: non_constant_identifier_names

@immutable
class LexerGrammar {
  // SECTION: lexicalGeneral

  static final Parser<void> NL =
      (char('\n') | char('\r') & char('\n').optional()).ignore();
  static final Parser<void> NLs = NL.star();

  // SECTION: separatorsAndOperations

  static final DOT = _operator('.', OperatorTokenType.dot);
  static final COMMA = _operator(',', OperatorTokenType.comma);

  static final LPAREN = _operator('(', OperatorTokenType.lparen);
  static final RPAREN = _operator(')', OperatorTokenType.rparen);
  static final LSQUARE = _operator('[', OperatorTokenType.lsquare);
  static final RSQUARE = _operator(']', OperatorTokenType.rsquare);

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

  static final EQUALS_EQUALS = _operator('=>=', OperatorTokenType.equalsEquals);
  static final EXCLAMATION_EQUALS =
      _operator('!=', OperatorTokenType.exclamationEquals);
  static final EQUALS_EQUALS_EQUALS =
      _operator('=>=>=', OperatorTokenType.equalsEqualsEquals);
  static final EXCLAMATION_EQUALS_EQUALS =
      _operator('!=>=', OperatorTokenType.exclamationEqualsEquals);

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

  // SECTION: literals

  static final IntegerLiteral = DecLiteral | HexLiteral | BinLiteral;

  static final DecLiteral =
      (_DecDigitNoZero & _DecDigitOrSeparator.star() & _DecDigit | _DecDigit)
          .tokenize((lexeme, span) =>
              IntegerLiteralToken(int.parse(lexeme), span: span));

  static final _DecDigits =
      _DecDigit & _DecDigitOrSeparator.star() & _DecDigit | _DecDigit;

  static final _DecDigit = digit();
  static final _DecDigitNoZero = char('1') |
      char('2') |
      char('3') |
      char('4') |
      char('5') |
      char('6') |
      char('7') |
      char('8') |
      char('9');
  static final _DecDigitOrSeparator = _DecDigit | char('_').ignore();

  // TODO: RealLiteral, FloatLiteral, DoubleLiteral

  static final HexLiteral = (_hexLiteralPrefix.ignore() &
          _HexDigit &
          (_HexDigitOrSeparator.star() & _HexDigit).optional())
      .tokenize<IntegerLiteralToken>((lexeme, span) {
    return IntegerLiteralToken(int.parse(lexeme, radix: 16), span: span);
  });
  static final _hexLiteralPrefix = char('0') & (char('x') | char('X'));

  static final _HexDigit = _DecDigit | _hexLettersLower | _hexLettersUpper;
  static final _hexLettersLower =
      char('a') | char('b') | char('c') | char('d') | char('e') | char('f');
  static final _hexLettersUpper =
      char('A') | char('B') | char('C') | char('D') | char('E') | char('F');
  static final _HexDigitOrSeparator = _HexDigit | char('_').ignore();

  static final BinLiteral = (_binLiteralPrefix.ignore() &
          _BinDigit &
          (_BinDigitOrSeparator.star() & _BinDigit).optional())
      .tokenize<IntegerLiteralToken>((lexeme, span) {
    return IntegerLiteralToken(int.parse(lexeme, radix: 2), span: span);
  });
  static final _binLiteralPrefix = char('0') & (char('b') | char('B'));

  static final _BinDigit = char('0') | char('1');
  static final _BinDigitOrSeparator = _BinDigit | char('_').ignore();

  static final BooleanLiteral = (string('true') | string('false')).tokenize(
      (lexeme, span) => BooleanLiteralToken(lexeme == 'true', span: span));

  // SECTION: lexicalIdentifiers

  static final Identifier =
      ((Letter | char('_')) & (Letter | char('_') | _DecDigit).star())
          .tokenize((lexeme, span) => SimpleIdentifier(lexeme, span: span));

  // SECTION: characters

  static final Letter = letter();
}

typedef LiteralMapper<T extends Token> = T Function(
  String lexeme,
  SourceSpan span,
);

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

  Parser<String> operator &(Parser<String> other) =>
      SequenceParser([this, other]).flatten();

  Parser<String> star() =>
      PossessiveRepeatingParser(this, 0, unbounded).flatten();
}
