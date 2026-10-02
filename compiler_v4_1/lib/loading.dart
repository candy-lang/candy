import 'package:supernova/supernova.dart';
import 'package:supernova/supernova_io.dart';

import 'context.dart';
import 'file.dart';
import 'hir/hir/ids.dart';
import 'lexer/lexer.dart';
import 'lexer/token.dart';
import 'lowering/modules.dart';
import 'parser/parser.dart';

class const LoadedPackage(final Context context, final List<Module> modules);

/// Loads the package rooted at [root] plus every package reachable through
/// `use` lines. Loading only what is reachable keeps an unrelated broken
/// package in the same directory from breaking the build.
LoadedPackage loadPackage(Directory root, {FileOverlay? overlay}) {
  final entry = Package(name: root.name, rootDirectory: root);

  // A file inside a package sits alongside its siblings; a loose file's own
  // directory is itself the packages directory, so sibling packages — Core
  // above all — stay reachable either way.
  final isInPackage = (root ~/ '_package.candy').existsSync();
  final workspace = Workspace({
    entry.name: entry,
    ...Workspace.discover(isInPackage ? root.parent : root).packages,
  });

  final context = Context();
  final modules = <Module>[];
  final loaded = <String>{};

  // Core is always loaded, not just when imported: its public declarations are
  // ambient in every module.
  final pending = <String>[entry.name, ModuleId.corePackageName];

  while (pending.isNotEmpty) {
    final name = pending.removeLast();
    if (!loaded.add(name)) continue;

    final package = workspace[name] ?? (name == entry.name ? entry : null);
    if (package == null) continue;

    for (final packageFile in package.discoverFiles()) {
      final module = parseModule(
        context,
        package,
        packageFile,
        overlay: overlay,
      );
      modules.add(module);
      for (final use in module.ast.uses) {
        final rootSegment = use.path.firstOrNull;
        if (rootSegment == null) continue;
        if (rootSegment.kind is TokenKind_PackageKeyword) continue;
        pending.add(rootSegment.rawText);
      }
    }
  }

  return LoadedPackage(context, modules);
}

Module parseModule(
  Context context,
  Package package,
  File file, {
  FileOverlay? overlay,
}) {
  final source = overlay?[file] ?? file.readAsStringSync();
  final sourceFile = SourceFile(file, source);
  final tokens = Lexer.lex(context, sourceFile, source);
  return Module(
    id: package.moduleIdFor(file),
    file: sourceFile,
    ast: Parser.parse(context, sourceFile, tokens),
  );
}

/// A Candy package is marked by a `_package.candy` file, which is the root
/// folder for resolving imports. Falls back to the file's own directory if
/// no package is found, so single-file invocations still work.
Directory findPackageRoot(File file) {
  for (var directory = file.absolute.parent; ;) {
    if ((directory ~/ '_package.candy').existsSync()) return directory;

    final parent = directory.parent;
    if (parent.path == directory.path) return file.absolute.parent;

    directory = parent;
  }
}

/// Unsaved file contents, keyed by absolute path. They shadow what's on disk so
/// tools like the language server can analyze an editor's buffer.
class FileOverlay {
  final _contents = <String, String>{};

  String? operator [](File file) => _contents[_keyOf(file)];
  void operator []=(File file, String content) =>
      _contents[_keyOf(file)] = content;

  void remove(File file) => _contents.remove(_keyOf(file));

  static String _keyOf(File file) => file.absolute.path;
}
