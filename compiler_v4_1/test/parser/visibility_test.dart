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

  AstVisibility visibilityOf(AstDeclaration declaration) =>
      switch (declaration) {
        AstDeclaration_Struct(:final struct) => struct.visibility,
        AstDeclaration_Enum(:final enum_) => enum_.visibility,
        AstDeclaration_Trait(:final trait) => trait.visibility,
        AstDeclaration_Assignment(:final assignment) => assignment.visibility,
        AstDeclaration_Function(:final function) => function.visibility,
        AstDeclaration_Impl() => .private,
      };

  test('defaults to private', () {
    final result = parse('fun f() {}');
    expect(result.errors, isEmpty);
    expect(visibilityOf(result.ast.declarations.single), AstVisibility.private);
  });

  test('parses visibility on every declaration kind', () {
    final result = parse('''
      package struct S {}
      public enum E { a }
      package trait T {}
      public let x = 1
      package fun f() {}
    ''');
    expect(result.errors, isEmpty);
    expect(result.ast.declarations.map(visibilityOf), [
      AstVisibility.package,
      AstVisibility.public,
      AstVisibility.package,
      AstVisibility.public,
      AstVisibility.package,
    ]);
  });

  test('parses visibility on struct fields', () {
    final result = parse('struct S { public a: Int, package b: Int, c: Int }');
    expect(result.errors, isEmpty);
    final struct =
        (result.ast.declarations.single as AstDeclaration_Struct).struct;
    final kind = struct.kind as AstStructKind_UserDefined;
    expect(kind.fields.map((it) => it.visibility), [
      AstVisibility.public,
      AstVisibility.package,
      AstVisibility.private,
    ]);
  });

  test('reports visibility on an impl', () {
    final result = parse('trait T {}\nstruct S {}\npublic impl S: T {}');
    expect(result.errors, ["An `impl` can't have a visibility modifier."]);
  });

  test('reports visibility on a trait function', () {
    final result = parse('trait T { public fun f(self: Self) }');
    expect(result.errors, [
      "A function in a trait or a trait impl can't have a visibility modifier.",
    ]);
  });

  test('reports visibility on a trait impl function', () {
    final result = parse(
      'trait T {}\nstruct S {}\nimpl S: T { public fun f(self: Self) {} }',
    );
    expect(result.errors, [
      "A function in a trait or a trait impl can't have a visibility modifier.",
    ]);
  });

  test('parses visibility on an inherent impl function', () {
    final result = parse('''
      struct S {}
      impl S {
        public fun f(self: Self) {}
        package fun g(self: Self) {}
        fun h(self: Self) {}
      }
    ''');
    expect(result.errors, isEmpty);
    final impl = (result.ast.declarations.last as AstDeclaration_Impl).impl;
    expect(impl.functions.map((it) => it.visibility), [
      AstVisibility.public,
      AstVisibility.package,
      AstVisibility.private,
    ]);
  });

  test('reports visibility on an associated type', () {
    final result = parse('trait T { public type A }');
    expect(result.errors, [
      "An associated type can't have a visibility modifier.",
    ]);
  });
}
