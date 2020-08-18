import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import '../source_span.dart';
import '../syntactic_entity.dart';

part 'token.freezed.dart';

class Token extends SyntacticEntity {
  const Token({
    @required this.span,
  }) : assert(span != null);

  @override
  final SourceSpan span;
}

@freezed
abstract class OperatorToken extends Token with _$OperatorToken {
  const factory OperatorToken(
    OperatorTokenType type, {
    @required SourceSpan span,
  }) = _OperatorToken;
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

class LiteralToken<T> extends Token {
  const LiteralToken(
    this.value, {
    @required SourceSpan span,
  }) : super(span: span);

  final T value;
}

class IntegerLiteralToken extends LiteralToken<int> {
  const IntegerLiteralToken(
    // ignore: avoid_positional_boolean_parameters
    int value, {
    @required SourceSpan span,
  })  : assert(value != null),
        super(value, span: span);
}

class BooleanLiteralToken extends LiteralToken<bool> {
  const BooleanLiteralToken(
    // ignore: avoid_positional_boolean_parameters
    bool value, {
    @required SourceSpan span,
  })  : assert(value != null),
        super(value, span: span);
}

class NullLiteralToken extends LiteralToken<void> {
  const NullLiteralToken({
    @required SourceSpan span,
  }) : super(null, span: span);
}

class SimpleIdentifier extends Token {
  const SimpleIdentifier(
    this.name, {
    @required SourceSpan span,
  })  : assert(name != null),
        super(span: span);

  final String name;
}
