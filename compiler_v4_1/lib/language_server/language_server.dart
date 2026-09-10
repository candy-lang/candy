import 'dart:async';
import 'dart:io';

import 'package:lsp_server/lsp_server.dart';

import '../error.dart';
import '../file.dart';
import '../hir/ast_to_hir.dart';
import '../loading.dart';
import '../position.dart';

/// Serves the Language Server Protocol over stdin/stdout.
///
/// Only document sync and diagnostics are implemented so far.
Future<void> runLanguageServer() =>
    LanguageServer(Connection(stdin, stdout)).run();

class LanguageServer {
  LanguageServer(this._connection);

  static const _debounce = Duration(milliseconds: 300);

  final Connection _connection;

  final _overlay = FileOverlay();
  final _openFiles = <Uri, File>{};

  /// What we last published diagnostics for, so we can clear the files that
  /// became error-free instead of leaving stale squiggles behind.
  final _publishedUris = <Uri>{};

  Timer? _pendingAnalysis;

  Future<void> run() async {
    _connection
      ..onInitialize(_initialize)
      ..onDidOpenTextDocument(_didOpen)
      ..onDidChangeTextDocument(_didChange)
      ..onDidSaveTextDocument(_didSave)
      ..onDidCloseTextDocument(_didClose)
      ..onShutdown(() async => _pendingAnalysis?.cancel())
      ..onExit(() async => exit(0));

    await _connection.listen();
  }

  Future<InitializeResult> _initialize(InitializeParams params) async =>
      InitializeResult(
        capabilities: ServerCapabilities(
          textDocumentSync: Either2.t2(
            TextDocumentSyncOptions(
              openClose: true,
              // Full sync: re-parsing a whole file is cheap, and incremental
              // sync would need us to maintain the document text ourselves.
              change: TextDocumentSyncKind.Full,
              save: const Either2.t1(true),
            ),
          ),
        ),
        serverInfo: InitializeResultServerInfo(name: 'Candy Language Server'),
      );

  Future<void> _didOpen(DidOpenTextDocumentParams params) async {
    final document = params.textDocument;
    final file = _fileOf(document.uri);
    _openFiles[document.uri] = file;
    _overlay[file] = document.text;
    _scheduleAnalysis();
  }

  Future<void> _didChange(DidChangeTextDocumentParams params) async {
    final change = params.contentChanges.lastOrNull;
    if (change == null) return;

    // We only ever announce full sync, so every change carries the whole file.
    final text = TextDocumentContentChangeEvent2.fromJson(
      change.toJson()! as Map<String, Object?>,
    ).text;
    _overlay[_fileOf(params.textDocument.uri)] = text;
    _scheduleAnalysis();
  }

  Future<void> _didSave(DidSaveTextDocumentParams params) async =>
      _scheduleAnalysis();

  Future<void> _didClose(DidCloseTextDocumentParams params) async {
    final file = _openFiles.remove(params.textDocument.uri);
    if (file != null) _overlay.remove(file);
    _scheduleAnalysis();
  }

  /// Keystrokes arrive far faster than a package can be analyzed, so we only
  /// start once the typing pauses.
  void _scheduleAnalysis() {
    _pendingAnalysis?.cancel();
    _pendingAnalysis = Timer(_debounce, _analyze);
  }

  void _analyze() {
    final diagnostics = <Uri, List<Diagnostic>>{};
    for (final root in _packageRootsOfOpenFiles()) {
      for (final error in _errorsIn(root)) {
        final uri = error.span.file.file.absolute.uri;
        diagnostics.putIfAbsent(uri, () => []).add(_diagnosticOf(error));
      }
    }

    for (final uri in _publishedUris) {
      diagnostics.putIfAbsent(uri, () => []);
    }
    _publishedUris
      ..clear()
      ..addAll(diagnostics.keys.where((it) => diagnostics[it]!.isNotEmpty));

    for (final MapEntry(key: uri, value: fileDiagnostics)
        in diagnostics.entries) {
      _connection.sendDiagnostics(
        PublishDiagnosticsParams(uri: uri, diagnostics: fileDiagnostics),
      );
    }
  }

  /// A crash in the compiler must not take the server down with it: the editor
  /// would lose all diagnostics and never get them back.
  List<CompilerError> _errorsIn(Directory root) {
    try {
      final loaded = loadPackage(root, overlay: _overlay);
      astToHir(loaded.context, loaded.modules);
      return loaded.context.errors;
    } catch (error, stackTrace) {
      _connection.console.error(
        'Analyzing ${root.path} failed: $error\n$stackTrace',
      );
      return const [];
    }
  }

  /// Analyzing one root covers every file in it, so files sharing a package are
  /// only analyzed once.
  List<Directory> _packageRootsOfOpenFiles() {
    final roots = <String, Directory>{};
    for (final file in _openFiles.values) {
      final root = findPackageRoot(file);
      roots[root.absolute.path] = root;
    }
    return roots.values.toList();
  }

  Diagnostic _diagnosticOf(CompilerError error) => Diagnostic(
    range: Range(
      start: _positionOf(error.span.file, error.span.span.start),
      end: _positionOf(error.span.file, error.span.span.end),
    ),
    severity: DiagnosticSeverity.Error,
    source: 'candy',
    message: error.message,
  );

  Position _positionOf(SourceFile file, Offset offset) {
    final (:line, :column) = file.locationOf(offset);
    return Position(line: line - 1, character: column - 1);
  }

  static File _fileOf(Uri uri) => File.fromUri(uri).absolute;
}
