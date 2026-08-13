import 'dart:io';

import 'package:supernova/supernova.dart';

import '../ast.dart';
import '../file.dart';
import '../hir/hir/builtins.dart';
import '../hir/hir/ids.dart';

class Module {
  const Module({required this.id, required this.file, required this.ast});

  final ModuleId id;
  final SourceFile file;
  final Ast ast;
}

class Package {
  const Package({required this.name, required this.rootDirectory});

  final String name;
  final Directory rootDirectory;

  /// Files outside [rootDirectory] fall back to their basename, so
  /// single-file invocations like `hir_check foo.candy` work.
  ModuleId moduleIdFor(File file) {
    final rootPath = _normalize(rootDirectory.absolute.path);
    final filePath = _normalize(file.absolute.path);
    final relative = filePath.startsWith('$rootPath/')
        ? filePath.substring(rootPath.length + 1)
        : filePath.split('/').last;
    final withoutExtension = relative.endsWith('.candy')
        ? relative.substring(0, relative.length - '.candy'.length)
        : relative;
    return ModuleId(
      packageName: name,
      path: withoutExtension.split('/').where((it) => it.isNotEmpty).toList(),
    );
  }

  static String _normalize(String path) =>
      path.endsWith('/') ? path.substring(0, path.length - 1) : path;

  List<File> discoverFiles() => rootDirectory
      .listSync(recursive: true)
      .whereType<File>()
      .where((it) => it.path.endsWith('.candy'))
      .sortedBy((it) => it.path)
      .toList();
}

/// A name can refer to several functions at once, but to at most one type,
/// trait, or assignment.
@immutable
sealed class NamedDeclarations {
  const NamedDeclarations();
}

class NamedType extends NamedDeclarations {
  const NamedType(this.id);
  final DeclarationId id;
}

class NamedTrait extends NamedDeclarations {
  const NamedTrait(this.id);
  final DeclarationId id;
}

class NamedAssignment extends NamedDeclarations {
  const NamedAssignment(this.id);
  final DeclarationId id;
}

class NamedFunctions extends NamedDeclarations {
  const NamedFunctions(this.ids);
  final List<DeclarationId> ids;
}

/// Every name lookup during lowering goes through [lookup], so adding `use`
/// syntax later means filling [imports] rather than changing call sites.
class ImportScope {
  ImportScope({
    required this.moduleDeclarations,
    required this.imports,
    required this.prelude,
  });

  final Map<String, NamedDeclarations> moduleDeclarations;

  /// Explicit and glob imports share one layer for now.
  final Map<String, NamedDeclarations> imports;

  final Map<String, NamedDeclarations> prelude;

  /// Innermost layer wins, except for functions: overloads from every layer
  /// are merged so a local `plus` doesn't hide an imported `plus` with a
  /// different signature. Overload resolution then ranks them by layer.
  NamedDeclarations? lookup(String name) {
    final layers = [moduleDeclarations, imports, prelude];
    final found = layers.map((it) => it[name]).nonNulls.toList();
    if (found.isEmpty) return null;

    final first = found.first;
    if (first is! NamedFunctions) return first;

    return NamedFunctions([
      for (final declarations in found)
        if (declarations is NamedFunctions) ...declarations.ids,
    ]);
  }

  /// Lowest is innermost.
  int layerOf(DeclarationId id) {
    bool contains(Map<String, NamedDeclarations> layer) => layer.values.any(
      (it) => switch (it) {
        NamedFunctions(:final ids) => ids.contains(id),
        NamedType(id: final other) ||
        NamedTrait(id: final other) ||
        NamedAssignment(id: final other) => other == id,
      },
    );

    if (contains(moduleDeclarations)) return 0;
    if (contains(imports)) return 1;
    return 2;
  }
}

Map<String, NamedDeclarations> buildPrelude() {
  final byName = <String, List<DeclarationId>>{};
  for (final function in BuiltinFunction.values) {
    byName.putIfAbsent(function.candyName, () => []).add(function.id);
  }
  return {
    for (final MapEntry(:key, :value) in byName.entries)
      key: NamedFunctions(value),
  };
}
