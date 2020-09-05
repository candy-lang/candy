import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import '../source_span.dart';
import '../syntactic_entity.dart';

part 'token.freezed.dart';

abstract class Token extends SyntacticEntity {}

@freezed
abstract class OperatorToken extends Token with _$OperatorToken {
  const factory OperatorToken(
    OperatorTokenType type, {
    @required SourceSpan span,
  }) = _OperatorToken;
}

@freezed
abstract class KeywordToken extends Token with _$KeywordToken {
  // ignore: non_constant_identifier_names
  const factory KeywordToken.class_({@required SourceSpan span}) =
      ClassKeywordToken;
  // Declarations:
  const factory KeywordToken.fun({@required SourceSpan span}) = FunKeywordToken;
  const factory KeywordToken.let({@required SourceSpan span}) = LetKeywordToken;
  const factory KeywordToken.mut({@required SourceSpan span}) = MutKeywordToken;
  const factory KeywordToken.get({@required SourceSpan span}) = GetKeywordToken;
  const factory KeywordToken.set({@required SourceSpan span}) = SetKeywordToken;
  // Statements:
  // ignore: non_constant_identifier_names
  const factory KeywordToken.if_({@required SourceSpan span}) = IfKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.else_({@required SourceSpan span}) =
      ElseKeywordToken;
}

@freezed
abstract class ModifierToken extends Token with _$ModifierToken {
  const factory ModifierToken.external({@required SourceSpan span}) =
      _ExternalModifierToken;
  const factory ModifierToken.abstract({@required SourceSpan span}) =
      _AbstractModifierToken;
  // ignore: non_constant_identifier_names
  const factory ModifierToken.const_({@required SourceSpan span}) =
      _ConstModifierToken;
}

abstract class LiteralToken<T> extends Token {
  T get value;
}

@freezed
abstract class IntegerLiteralToken extends LiteralToken<int>
    with _$IntegerLiteralToken {
  const factory IntegerLiteralToken(
    int value, {
    @required SourceSpan span,
  }) = _IntegerLiteralToken;
}

@freezed
abstract class BooleanLiteralToken extends LiteralToken<bool>
    with _$BooleanLiteralToken {
  const factory BooleanLiteralToken(
    // ignore: avoid_positional_boolean_parameters
    bool value, {
    @required SourceSpan span,
  }) = _BooleanLiteralToken;
}

@freezed
abstract class IdentifierToken extends Token with _$IdentifierToken {
  const factory IdentifierToken(
    String name, {
    @required SourceSpan span,
  }) = _IdentifierToken;
}

enum OperatorTokenType {
  /// `.`
  dot,

  /// `,`
  comma,

  /// `:`
  colon,

  /// `=>`
  equalsGreater,

  /// `(`
  lparen,

  /// `)`
  rparen,

  /// `[`
  lsquare,

  /// `]`
  rsquare,

  /// `{`
  lcurl,

  /// `}`
  rcurl,

  /// `<`
  langle,

  /// `>`
  rangle,

  /// `++`
  plusPlus,

  /// `--`
  minusMinus,

  /// `?`
  question,

  /// `!`
  exclamation,

  /// `~`
  tilde,

  /// `*`
  asterisk,

  /// `/`
  slash,

  /// `~/`
  tildeSlash,

  /// `%`
  percent,

  /// `+`
  plus,

  /// `-`
  minus,

  /// `<<`
  lessLess,

  /// `>>`
  greaterGreater,

  /// `>>>`
  greaterGreaterGreater,

  /// `&`
  ampersand,

  /// `^`
  caret,

  /// `|`
  bar,

  /// `as`
  as,

  /// `as?`
  asSafe,

  /// `..`
  dotDot,

  /// `..=`
  dotDotEquals,

  /// `in`
  // ignore: constant_identifier_names
  in_,

  /// `!in`
  exclamationIn,

  /// `is`
  // ignore: constant_identifier_names
  is_,

  /// `!is`
  exclamationIs,

  /// `<`
  less,

  /// `<=`
  lessEquals,

  /// `>`
  greater,

  /// `>=`
  greaterEquals,

  /// `==`
  equalsEquals,

  /// `!=`
  exclamationEquals,

  /// `===`
  equalsEqualsEquals,

  /// `!==`
  exclamationEqualsEquals,

  /// `&&`
  ampersandAmpersand,

  /// `||`
  barBar,

  /// `->`
  dashGreater,

  /// `<-`
  lessDash,

  /// `...`
  dotDotDot,

  /// `=`
  equals,

  /// `*=`
  asteriskEquals,

  /// `/=`
  slashEquals,

  /// `~/=`
  tildeSlashEquals,

  /// `%=`
  percentEquals,

  /// `+=`
  plusEquals,

  /// `-=`
  minusEquals,

  /// `&=`
  ampersandEquals,

  /// `|=`
  barEquals,

  /// `^=`
  caretEquals,

  /// `&&=`
  ampersandAmpersandEquals,

  /// `||=`
  barBarEquals,

  /// `<<=`
  lessLessEquals,

  /// `>>=`
  greaterGreaterEquals,

  /// `>>>=`
  greaterGreaterGreaterEquals,
}
