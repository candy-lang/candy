import 'package:supernova/supernova.dart';

import '../position.dart';

part 'token.freezed.dart';
part 'token.g.dart';

@freezed
sealed class Token with _$Token implements ToJsonMap {
  const factory Token({
    required SourceSpan span,
    required TokenKind kind,
    @protected String? text,
  }) = _Token;

  factory Token.fromJson(Map<String, dynamic> json) {
    return Token(
      span: SourceSpan.fromJson(json['span'] as Map<String, dynamic>),
      kind: TokenKind.fromJson(json['kind'] as Map<String, dynamic>),
      text: json['rawText']! as String,
    );
  }
  const Token._();

  String get rawText =>
      text ??
      span.file.content.substring(span.span.start.value, span.span.end.value);

  @override
  Map<String, dynamic> toJson() => {
    'span': span.toJson(),
    'kind': kind.toJson(),
    'rawText': rawText,
  };
}

@freezed
sealed class TokenKind with _$TokenKind {
  const factory TokenKind.whitespace(String whitespace) = TokenKind_Whitespace;
  const factory TokenKind.lineBreak() = TokenKind_LineBreak;
  const factory TokenKind.comment(String comment) = TokenKind_Comment;

  const factory TokenKind.openingParenthesis() = TokenKind_OpeningParenthesis;
  const factory TokenKind.closingParenthesis() = TokenKind_ClosingParenthesis;
  const factory TokenKind.openingBrace() = TokenKind_OpeningBrace;
  const factory TokenKind.closingBrace() = TokenKind_ClosingBrace;
  const factory TokenKind.openingBracket() = TokenKind_OpeningBracket;
  const factory TokenKind.closingBracket() = TokenKind_ClosingBracket;
  const factory TokenKind.colon() = TokenKind_Colon;
  const factory TokenKind.comma() = TokenKind_Comma;
  const factory TokenKind.dot() = TokenKind_Dot;
  const factory TokenKind.equal() = TokenKind_Equal;
  const factory TokenKind.arrow() = TokenKind_Arrow;
  const factory TokenKind.plus() = TokenKind_Plus;

  const factory TokenKind.identifier() = TokenKind_Identifier;

  const factory TokenKind.int(int value) = TokenKind_Int;

  const factory TokenKind.openingQuote() = TokenKind_OpeningQuote;
  const factory TokenKind.regularStringPart() = TokenKind_RegularStringPart;
  const factory TokenKind.escapedChar(String value) = TokenKind_EscapedChar;
  const factory TokenKind.textInterpolationStart() =
      TokenKind_TextInterpolationStart;
  const factory TokenKind.textInterpolationEnd() =
      TokenKind_TextInterpolationEnd;
  const factory TokenKind.closingQuote() = TokenKind_ClosingQuote;

  const factory TokenKind.builtinKeyword() = TokenKind_BuiltinKeyword;
  const factory TokenKind.structKeyword() = TokenKind_StructKeyword;
  const factory TokenKind.enumKeyword() = TokenKind_EnumKeyword;
  const factory TokenKind.traitKeyword() = TokenKind_TraitKeyword;
  const factory TokenKind.implKeyword() = TokenKind_ImplKeyword;
  const factory TokenKind.typeKeyword() = TokenKind_TypeKeyword;
  const factory TokenKind.whereKeyword() = TokenKind_WhereKeyword;
  const factory TokenKind.funKeyword() = TokenKind_FunKeyword;
  const factory TokenKind.letKeyword() = TokenKind_LetKeyword;
  const factory TokenKind.switchKeyword() = TokenKind_SwitchKeyword;
  const factory TokenKind.useKeyword() = TokenKind_UseKeyword;
  const factory TokenKind.packageKeyword() = TokenKind_PackageKeyword;
  const factory TokenKind.publicKeyword() = TokenKind_PublicKeyword;

  const factory TokenKind.endOfFile() = TokenKind_EndOfFile;

  factory TokenKind.fromJson(Map<String, dynamic> json) =>
      _$TokenKindFromJson(json);
  const TokenKind._();

  bool get isCommentOrWhitespace =>
      this is TokenKind_Comment || this is TokenKind_Whitespace;
}
