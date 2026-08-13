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

  /// Several modules in one package named `test`. Keys are slash-separated
  /// module paths.
  factory LoweringHarness.modules(Map<String, String> sources) {
    final harness = LoweringHarness._(sources.values.join('\n'), Context());
    final modules = [
      for (final MapEntry(key: path, value: source) in sources.entries)
        harness._parseModule(path, source),
    ];
    harness
      ..declarations = collectDeclarations(harness.context, modules)
      ..signatures = lowerSignatures(harness.context, harness.declarations);
    return harness;
  }

  /// Like [LoweringHarness.modules], but runs bodies too, so name resolution
  /// inside function bodies is actually exercised. One module must define
  /// `main`.
  factory LoweringHarness.fullModules(Map<String, String> sources) {
    final harness = LoweringHarness._(sources.values.join('\n'), Context());
    harness.hir = astToHir(harness.context, [
      for (final MapEntry(key: path, value: source) in sources.entries)
        harness._parseModule(path, source),
    ]);
    return harness;
  }

  /// Several packages at once. Outer keys are package names, inner keys are
  /// module paths within that package.
  factory LoweringHarness.packages(Map<String, Map<String, String>> sources) {
    final harness = LoweringHarness._(
      [for (final package in sources.values) ...package.values].join('\n'),
      Context(),
    );
    final modules = [
      for (final MapEntry(key: package, value: files) in sources.entries)
        for (final MapEntry(key: path, value: source) in files.entries)
          harness._parseModule(path, source, packageName: package),
    ];
    harness
      ..declarations = collectDeclarations(harness.context, modules)
      ..signatures = lowerSignatures(harness.context, harness.declarations);
    return harness;
  }

  /// Like [LoweringHarness.packages], but runs bodies too. One module must
  /// define `main`.
  factory LoweringHarness.fullPackages(
    Map<String, Map<String, String>> sources,
  ) {
    final harness = LoweringHarness._(
      [for (final package in sources.values) ...package.values].join('\n'),
      Context(),
    );
    harness.hir = astToHir(harness.context, [
      for (final MapEntry(key: package, value: files) in sources.entries)
        for (final MapEntry(key: path, value: source) in files.entries)
          harness._parseModule(path, source, packageName: package),
    ]);
    return harness;
  }

  /// Lexes and parses only, keeping whatever errors that produced.
  factory LoweringHarness.parseOnly(String source) {
    final harness = LoweringHarness._(source, Context());
    final file = SourceFile(File('test.candy'), source);
    Parser.parse(
      harness.context,
      file,
      Lexer.lex(harness.context, file, source),
    );
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

  Module _parseModule(
    String path,
    String source, {
    String packageName = ModuleId.corePackageName,
  }) {
    final file = SourceFile(File('$path.candy'), source);
    final ast = Parser.parse(context, file, Lexer.lex(context, file, source));
    expect(
      context.errors,
      isEmpty,
      reason: 'Module `$path` failed to lex or parse.',
    );
    return Module(
      id: ModuleId(packageName: packageName, path: path.split('/')),
      file: file,
      ast: ast,
    );
  }

  Module _parse() {
    final file = SourceFile(File('test.candy'), source);
    final ast = Parser.parse(context, file, Lexer.lex(context, file, source));
    expect(
      context.errors,
      isEmpty,
      reason: 'The source failed to lex or parse.',
    );
    return Module(
      id: const ModuleId(packageName: ModuleId.corePackageName, path: ['test']),
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
use Builtins
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
