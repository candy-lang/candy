import 'dart:io';

import 'package:compiler_v4_1/ast.dart';
import 'package:compiler_v4_1/context.dart';
import 'package:compiler_v4_1/file.dart';
import 'package:compiler_v4_1/lexer/lexer.dart';
import 'package:compiler_v4_1/parser/parser.dart';
import 'package:test/test.dart';

void main() {
  ({Ast ast, List<String> errors}) parse(String source) {
    final context = Context();
    final file = SourceFile(File('test.candy'), source);
    final ast = Parser.parse(context, file, Lexer.lex(context, file, source));
    return (ast: ast, errors: context.errors.map((it) => it.message).toList());
  }

  List<String> pathsOf(Ast ast) => [
    for (final use in ast.uses) use.path.map((it) => it.rawText).join('.'),
  ];

  test('parses a single-segment path', () {
    final result = parse('use core');
    expect(result.errors, isEmpty);
    expect(pathsOf(result.ast), ['core']);
  });

  test('parses a multi-segment path', () {
    final result = parse('use core.collections.list');
    expect(result.errors, isEmpty);
    expect(pathsOf(result.ast), ['core.collections.list']);
  });

  test('parses `package` as the path root', () {
    final result = parse('use package.lowering.bodies');
    expect(result.errors, isEmpty);
    expect(pathsOf(result.ast), ['package.lowering.bodies']);
  });

  test('parses several use lines before declarations', () {
    final result = parse('use core\nuse package.a\nfun f() {}');
    expect(result.errors, isEmpty);
    expect(pathsOf(result.ast), ['core', 'package.a']);
    expect(result.ast.declarations, hasLength(1));
  });

  test('reports a use after a declaration', () {
    final result = parse('fun f() {}\nuse core');
    expect(result.errors, ['`use` must come before all declarations.']);
  });

  test('reports a missing path', () {
    final result = parse('use');
    expect(result.errors, ['This `use` is missing a module path.']);
  });

  test('reports a missing segment after a dot', () {
    final result = parse('use core.');
    expect(result.errors, [
      'This `use` path is missing a segment after the dot.',
    ]);
  });
}
