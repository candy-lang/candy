import 'dart:io';

import 'package:compiler_v4_1/context.dart';
import 'package:compiler_v4_1/file.dart';
import 'package:compiler_v4_1/lexer/lexer.dart';
import 'package:compiler_v4_1/lexer/token.dart';
import 'package:test/test.dart';

void main() {
  List<TokenKind> kindsOf(String source) {
    final context = Context();
    final file = SourceFile(File('test.candy'), source);
    return Lexer.lex(context, file, source)
        .map((it) => it.kind)
        .where((it) => !it.isCommentOrWhitespace && it is! TokenKind_EndOfFile)
        .toList();
  }

  test('lexes the import and visibility keywords', () {
    expect(kindsOf('use package public'), const [
      TokenKind.useKeyword(),
      TokenKind.packageKeyword(),
      TokenKind.publicKeyword(),
    ]);
  });

  test('does not treat longer identifiers as keywords', () {
    expect(kindsOf('used packages publicly'), const [
      TokenKind.identifier(),
      TokenKind.identifier(),
      TokenKind.identifier(),
    ]);
  });
}
