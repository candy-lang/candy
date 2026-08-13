import 'dart:io';

import 'package:compiler_v4_1/context.dart';
import 'package:compiler_v4_1/error.dart';
import 'package:compiler_v4_1/file.dart';
import 'package:compiler_v4_1/hir/ast_to_hir.dart';
import 'package:compiler_v4_1/hir/hir/hir.dart';
import 'package:compiler_v4_1/hir/hir/ids.dart';
import 'package:compiler_v4_1/hir/hir/to_text.dart';
import 'package:compiler_v4_1/lexer/lexer.dart';
import 'package:compiler_v4_1/lowering/declarations.dart';
import 'package:compiler_v4_1/lowering/modules.dart';
import 'package:compiler_v4_1/lowering/signatures.dart';
import 'package:compiler_v4_1/parser/parser.dart';
import 'package:test/test.dart';

// ignore: avoid-top-level-members-in-tests
class LoweringHarness {
  LoweringHarness._(this.source, this.context);

  /// Stops after signature lowering, leaving bodies alone.
  factory LoweringHarness.of(String source) {
    final harness = LoweringHarness._(source, Context());
    final module = harness._parse();
    harness
      ..declarations = collectDeclarations(harness.context, [module])
      ..signatures = lowerSignatures(harness.context, harness.declarations);
    return harness;
  }

  /// Runs the whole pipeline, bodies included.
  factory LoweringHarness.full(String source) {
    final harness = LoweringHarness._(source, Context());
    harness.hir = astToHir(harness.context, [harness._parse()]);
    return harness;
  }

  final String source;
  final Context context;

  late final CollectedDeclarations declarations;
  late final Signatures signatures;
  late final Hir hir;

  Module _parse() {
    final file = SourceFile(File('test.candy'), source);
    final ast = Parser.parse(context, file, Lexer.lex(context, file, source));
    expect(
      context.errors,
      isEmpty,
      reason: 'The source failed to lex or parse.',
    );
    return Module(
      id: const ModuleId(packageName: 'test', path: ['test']),
      file: file,
      ast: ast,
    );
  }

  List<String> get errorMessages =>
      context.errors.map((it) => it.message).toList();

  String describeErrors() => context.errors.map(_describe).join('\n');

  String _describe(CompilerError error) {
    final start = error.span.span.start.value;
    final line = '\n'.allMatches(source.substring(0, start)).length + 1;
    return '  line $line: ${error.message}';
  }

  void expectNoErrors() =>
      expect(context.errors, isEmpty, reason: '\n${describeErrors()}');

  /// The signature of the top-level function named [name], as text.
  String signatureOf(String name) {
    final matches = signatures.functions.values
        .where((it) => !it.isBuiltin && it.signature.name == name)
        .toList();
    expect(matches, hasLength(1), reason: 'Expected exactly one `$name`.');
    return '${matches.single.signature}';
  }

  HirFunction functionNamed(String name) {
    final matches = hir.functions.values
        .where((it) => it.signature.name == name)
        .toList();
    expect(matches, hasLength(1), reason: 'Expected exactly one `$name`.');
    return matches.single;
  }

  /// The types of the named bindings in [name]'s body, in order.
  Map<String, String> bindingsOf(String name) {
    final body = functionNamed(name).body;
    if (body is! HirBodyOrBuiltin_Body) return {};
    return {
      for (final entry in body.body.expressions)
        if (entry.name != null) entry.name!: '${entry.expression.type}',
    };
  }

  /// The declaration each call in [name]'s body resolved to.
  List<String> callsIn(String name) {
    final body = functionNamed(name).body;
    if (body is! HirBodyOrBuiltin_Body) return [];
    return [
      for (final entry in body.body.expressions)
        if (entry.expression.kind case HirExpressionKind_Call(:final function))
          '$function',
    ];
  }

  String bodyTextOf(String name) => hirToText(
    Hir(
      solverEnvironment: hir.solverEnvironment,
      typeDeclarations: const {},
      traits: const {},
      impls: const [],
      assignments: const {},
      functions: {functionNamed(name).signature.id: functionNamed(name)},
      mainFunction: null,
    ),
  );
}

/// A minimal set of declarations that most tests need in scope.
// ignore: avoid-top-level-members-in-tests
const preludeSource = '''
struct Nothing {}
enum Never {}
struct Int = builtin
struct Text = builtin
struct List[T] = builtin
enum Bool { false_, true_ }
enum Ordering { less, equal, greater }
enum Maybe[T] { none, some: T }
enum Result[T, E] { ok: T, error: E }
''';
