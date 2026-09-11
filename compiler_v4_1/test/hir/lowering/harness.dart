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
import 'package:compiler_v4_1/position.dart';
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

  /// Every module's source, keyed by its path, so tests can look up offsets in
  /// the file a name actually lives in.
  final files = <String, SourceFile>{};

  late final CollectedDeclarations declarations;
  late final Signatures signatures;
  late final Hir hir;

  Module _parseModule(
    String path,
    String source, {
    String packageName = ModuleId.corePackageName,
  }) {
    final file = SourceFile(File('$path.candy'), source);
    files[path] = file;
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
    files['test'] = file;
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

  /// Where go-to-definition lands from the [occurrence]-th occurrence of
  /// [needle] in [path]'s source, each as `text@line`.
  List<String> definitionsAt(
    String needle, {
    int occurrence = 1,
    String path = 'test',
  }) => context.references
      .targetsAt(files[path]!.file, _offsetOf(needle, occurrence, path))
      .map(_describeTarget)
      .toList();

  /// The same `text@line` shape [definitionsAt] returns, for spelling out
  /// where a definition is expected to be without hard-coding line numbers.
  String location(String needle, {int occurrence = 1, String path = 'test'}) {
    final offset = _offsetOf(needle, occurrence, path);
    return '$needle@${_lineOf(files[path]!.content, offset.value)}';
  }

  Offset _offsetOf(String needle, int occurrence, String path) {
    final content = files[path]!.content;
    var offset = -1;
    for (var i = 0; i < occurrence; i++) {
      offset = content.indexOf(needle, offset + 1);
      expect(offset, isNot(-1), reason: 'No `$needle` #$occurrence in $path.');
    }
    return Offset(offset);
  }

  String _describeTarget(SourceSpan span) {
    final content = span.file.content;
    final start = span.span.start.value;
    final text = content.substring(start, span.span.end.value);
    return '$text@${_lineOf(content, start)}';
  }

  int _lineOf(String content, int offset) =>
      '\n'.allMatches(content.substring(0, offset)).length + 1;

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
