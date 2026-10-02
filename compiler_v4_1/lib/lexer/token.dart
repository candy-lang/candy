import 'package:supernova/supernova.dart';

import '../position.dart';

part 'token.freezed.dart';
part 'token.g.dart';

@freezed
sealed class const Token._() with _$Token implements ToJsonMap {
  const factory({
    required SourceSpan span,
    required TokenKind kind,
    @protected String? text,
  }) = _Token;

  factory fromJson(Map<String, dynamic> json) {
    return Token(
      span: SourceSpan.fromJson(json['span'] as Map<String, dynamic>),
      kind: TokenKind.fromJson(json['kind'] as Map<String, dynamic>),
      text: json['rawText']! as String,
    );
  }

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
sealed class const TokenKind._() with _$TokenKind {
  const factory whitespace(String whitespace) = TokenKind_Whitespace;
  const factory lineBreak() = TokenKind_LineBreak;
  const factory comment(String comment) = TokenKind_Comment;

  const factory openingParenthesis() = TokenKind_OpeningParenthesis;
  const factory closingParenthesis() = TokenKind_ClosingParenthesis;
  const factory openingBrace() = TokenKind_OpeningBrace;
  const factory closingBrace() = TokenKind_ClosingBrace;
  const factory openingBracket() = TokenKind_OpeningBracket;
  const factory closingBracket() = TokenKind_ClosingBracket;
  const factory colon() = TokenKind_Colon;
  const factory comma() = TokenKind_Comma;
  const factory dot() = TokenKind_Dot;
  const factory equal() = TokenKind_Equal;
  const factory arrow() = TokenKind_Arrow;
  const factory plus() = TokenKind_Plus;

  const factory identifier() = TokenKind_Identifier;

  const factory int(int value) = TokenKind_Int;

  const factory openingQuote() = TokenKind_OpeningQuote;
  const factory regularStringPart() = TokenKind_RegularStringPart;
  const factory escapedChar(String value) = TokenKind_EscapedChar;
  const factory textInterpolationStart() = TokenKind_TextInterpolationStart;
  const factory textInterpolationEnd() = TokenKind_TextInterpolationEnd;
  const factory closingQuote() = TokenKind_ClosingQuote;

  const factory builtinKeyword() = TokenKind_BuiltinKeyword;
  const factory structKeyword() = TokenKind_StructKeyword;
  const factory enumKeyword() = TokenKind_EnumKeyword;
  const factory traitKeyword() = TokenKind_TraitKeyword;
  const factory implKeyword() = TokenKind_ImplKeyword;
  const factory typeKeyword() = TokenKind_TypeKeyword;
  const factory whereKeyword() = TokenKind_WhereKeyword;
  const factory funKeyword() = TokenKind_FunKeyword;
  const factory letKeyword() = TokenKind_LetKeyword;
  const factory switchKeyword() = TokenKind_SwitchKeyword;
  const factory useKeyword() = TokenKind_UseKeyword;
  const factory packageKeyword() = TokenKind_PackageKeyword;
  const factory publicKeyword() = TokenKind_PublicKeyword;

  const factory endOfFile() = TokenKind_EndOfFile;

  factory fromJson(Map<String, dynamic> json) => _$TokenKindFromJson(json);

  bool get isCommentOrWhitespace =>
      this is TokenKind_Comment || this is TokenKind_Whitespace;
}
