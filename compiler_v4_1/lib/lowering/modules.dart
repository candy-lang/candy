import 'package:supernova/supernova.dart';
import 'package:supernova/supernova_io.dart';

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
    final segments = withoutExtension
        .split('/')
        .where((it) => it.isNotEmpty)
        .toList();
    if (segments.lastOrNull == '_') segments.removeLast();
    return ModuleId(packageName: name, path: segments);
  }

  static String _normalize(String path) =>
      path.endsWith('/') ? path.substring(0, path.length - 1) : path;

  /// A subdirectory with its own `_package.candy` is a separate package, so it
  /// is not part of this one.
  List<File> discoverFiles() {
    final files = <File>[];

    void walk(Directory directory) {
      for (final entity in directory.listSync()) {
        switch (entity) {
          case File():
            final name = entity.uri.pathSegments.last;
            if (name.endsWith('.candy') && name != '_package.candy') {
              files.add(entity);
            }
          case Directory():
            if (!File('${entity.path}/_package.candy').existsSync()) {
              walk(entity);
            }
          default:
        }
      }
    }

    walk(rootDirectory);
    return files.sortedBy((it) => it.path);
  }
}

/// Every package lives in one directory, and its name is its directory name.
/// A package is marked by an empty `_package.candy` file.
class Workspace {
  const Workspace(this.packages);

  factory Workspace.discover(Directory packagesDirectory) {
    final packages = <String, Package>{};
    for (final directory
        in packagesDirectory.listSync().whereType<Directory>()) {
      if (!(directory ~/ '_package.candy').existsSync()) continue;

      final name = _directoryName(directory);
      packages[name] = Package(name: name, rootDirectory: directory);
    }
    return Workspace(packages);
  }

  /// The fallback for a file with no `_package.candy` above it, so single-file
  /// invocations keep working.
  factory Workspace.single(Package package) =>
      Workspace({package.name: package});

  static String _directoryName(Directory directory) => Package._normalize(
    directory.absolute.path,
  ).split('/').lastWhere((it) => it.isNotEmpty);

  final Map<String, Package> packages;

  Package? operator [](String name) => packages[name];
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

/// Two imported modules contributed the same name. Reported where the name is
/// used, not at the `use` line, because there is no `hide` to resolve it with.
class NamedAmbiguous extends NamedDeclarations {
  const NamedAmbiguous(this.ids);
  final List<DeclarationId> ids;

  String get originsText {
    final modules = ids.map((it) => '${it.module}').toSet().toList()..sort();
    return modules.map((it) => '`$it`').join(' and ');
  }
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

  /// The `Core` package's public declarations, ambient in every module outside
  /// `Core` itself. Filled in the same pass as [imports], because Core's
  /// exports are not known until every module has been collected.
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
        NamedFunctions(:final ids) ||
        NamedAmbiguous(:final ids) => ids.contains(id),
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

/// The contents of the synthetic `Builtins` package.
Map<String, NamedDeclarations> buildBuiltins() {
  final byName = <String, List<DeclarationId>>{};
  for (final function in BuiltinFunction.values) {
    byName.putIfAbsent(function.candyName, () => []).add(function.id);
  }
  return {
    for (final MapEntry(:key, :value) in byName.entries)
      key: NamedFunctions(value),
  };
}
