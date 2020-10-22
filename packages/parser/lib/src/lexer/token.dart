import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import '../source_span.dart';
import '../syntactic_entity.dart';

part 'token.freezed.dart';

abstract class Token extends SyntacticEntity {
  const Token();
}

@freezed
abstract class OperatorToken extends Token with _$OperatorToken {
  const factory OperatorToken(OperatorTokenType type, {SourceSpan span}) =
      _OperatorToken;
}

@freezed
abstract class KeywordToken extends Token with _$KeywordToken {
  const factory KeywordToken.use({@required SourceSpan span}) = UseKeywordToken;
  const factory KeywordToken.crate({@required SourceSpan span}) =
      CrateKeywordToken;
  // Declarations:
  const factory KeywordToken.module({SourceSpan span}) = ModuleKeywordToken;
  const factory KeywordToken.trait({@required SourceSpan span}) =
      TraitKeywordToken;
  const factory KeywordToken.impl({@required SourceSpan span}) =
      ImplKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.class_({@required SourceSpan span}) =
      ClassKeywordToken;
  const factory KeywordToken.fun({@required SourceSpan span}) = FunKeywordToken;
  const factory KeywordToken.let({@required SourceSpan span}) = LetKeywordToken;
  const factory KeywordToken.get({@required SourceSpan span}) = GetKeywordToken;
  const factory KeywordToken.set({@required SourceSpan span}) = SetKeywordToken;
  // Expressions:
  // ignore: non_constant_identifier_names
  const factory KeywordToken.if_({@required SourceSpan span}) = IfKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.else_({@required SourceSpan span}) =
      ElseKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.return_({@required SourceSpan span}) =
      ReturnKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.break_({@required SourceSpan span}) =
      BreakKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.continue_({@required SourceSpan span}) =
      ContinueKeywordToken;
}

@freezed
abstract class ModifierToken extends Token with _$ModifierToken {
  const factory ModifierToken.public({@required SourceSpan span}) =
      PublicModifierToken;
  const factory ModifierToken.mut({@required SourceSpan span}) =
      MutModifierToken;
  const factory ModifierToken.static({@required SourceSpan span}) =
      StaticModifierToken;
  const factory ModifierToken.builtin({@required SourceSpan span}) =
      BuiltinModifierToken;
  const factory ModifierToken.external({@required SourceSpan span}) =
      ExternalModifierToken;
  const factory ModifierToken.override({@required SourceSpan span}) =
      OverrideModifierToken;
  // ignore: non_constant_identifier_names
  const factory ModifierToken.const_({@required SourceSpan span}) =
      ConstModifierToken;
}

abstract class LiteralToken<T> extends Token {
  const LiteralToken();

  T get value;
}

@freezed
abstract class BoolLiteralToken extends LiteralToken<bool>
    with _$BoolLiteralToken {
  const factory BoolLiteralToken(
    // ignore: avoid_positional_boolean_parameters
    bool value, {
    @required SourceSpan span,
  }) = _BoolLiteralToken;
}

@freezed
abstract class IntLiteralToken extends LiteralToken<int>
    with _$IntLiteralToken {
  const factory IntLiteralToken(
    int value, {
    @required SourceSpan span,
  }) = _IntLiteralToken;
}

@freezed
abstract class LiteralStringToken extends Token with _$LiteralStringToken {
  const factory LiteralStringToken(String value, {SourceSpan span}) =
      _LiteralStringToken;
}

@freezed
abstract class IdentifierToken extends Token with _$IdentifierToken {
  const factory IdentifierToken(String name, {SourceSpan span}) =
      _IdentifierToken;
}

enum OperatorTokenType {
  /// `.`
  dot,

  /// `,`
  comma,

  /// `:`
  colon,

  /// `#`
  hashtag,

  /// `"`
  quote,

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
