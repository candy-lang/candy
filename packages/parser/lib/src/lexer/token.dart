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
abstract class IdentifierToken extends Token with _$IdentifierToken {
  const factory IdentifierToken(
    String name, {
    @required SourceSpan span,
  }) = _IdentifierToken;
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

enum OperatorTokenType {
  /// `.`
  dot,

  /// `,`
  comma,

  /// `(`
  lparen,

  /// `)`
  rparen,

  /// `[`
  lsquare,

  /// `]`
  rsquare,

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
