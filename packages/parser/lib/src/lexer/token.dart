import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import '../source_span.dart';
import '../syntactic_entity.dart';
import '../visitor.dart';

part 'token.freezed.dart';

abstract class Token extends SyntacticEntity {
  const Token();
}

@freezed
abstract class OperatorToken extends Token implements _$OperatorToken {
  const factory OperatorToken(OperatorTokenType type, {SourceSpan span}) =
      _OperatorToken;
  const OperatorToken._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitOperatorToken(this);
}

@freezed
abstract class KeywordToken extends Token implements _$KeywordToken {
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
  const factory KeywordToken.loop({@required SourceSpan span}) =
      LoopKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.while_({@required SourceSpan span}) =
      WhileKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.for_({@required SourceSpan span}) =
      ForKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.return_({@required SourceSpan span}) =
      ReturnKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.break_({@required SourceSpan span}) =
      BreakKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.continue_({@required SourceSpan span}) =
      ContinueKeywordToken;
  // ignore: non_constant_identifier_names
  const factory KeywordToken.throw_({@required SourceSpan span}) =
      ThrowKeywordToken;

  const KeywordToken._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitKeywordToken(this);
}

@freezed
abstract class ModifierToken extends Token implements _$ModifierToken {
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
  const factory ModifierToken.data({@required SourceSpan span}) =
      DataModifierToken;
  const ModifierToken._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitModifierToken(this);
}

abstract class LiteralToken<T> extends Token {
  const LiteralToken();

  T get value;
}

@freezed
abstract class BoolLiteralToken extends LiteralToken<bool>
    implements _$BoolLiteralToken {
  const factory BoolLiteralToken(
    // ignore: avoid_positional_boolean_parameters
    bool value, {
    @required SourceSpan span,
  }) = _BoolLiteralToken;
  const BoolLiteralToken._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitBoolLiteralToken(this);
}

@freezed
abstract class IntLiteralToken extends LiteralToken<int>
    implements _$IntLiteralToken {
  const factory IntLiteralToken(
    int value, {
    @required SourceSpan span,
  }) = _IntLiteralToken;
  const IntLiteralToken._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitIntLiteralToken(this);
}

@freezed
abstract class LiteralStringToken extends Token
    implements _$LiteralStringToken {
  const factory LiteralStringToken(String value, {SourceSpan span}) =
      _LiteralStringToken;
  const LiteralStringToken._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitLiteralStringToken(this);
}

@freezed
abstract class IdentifierToken extends Token implements _$IdentifierToken {
  const factory IdentifierToken(String name, {SourceSpan span}) =
      _IdentifierToken;
  const IdentifierToken._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitIdentifierToken(this);
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
