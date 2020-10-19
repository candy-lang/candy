import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart' hide srcDirectoryName;
import 'package:dart_style/dart_style.dart';

import '../builtins.dart';
import '../constants.dart';
import 'declaration.dart';

final _dartFmt = DartFormatter();

final compileModule = Query<ModuleId, Unit>(
  'dart.compileModule',
  evaluateAlways: true,
  provider: (context, moduleId) {
    final module = getModuleDeclarationHir(context, moduleId);

    final library = dart.Library((b) {
      for (final declarationId in module.innerDeclarationIds) {
        final compiled = compileDeclaration(context, declarationId);
        if (compiled.isSome) b.body.add(compiled.value);
      }

      if (moduleId == ModuleId.corePrimitives) {
        b.body.addAll(DartBuiltinCompiler().compilePrimitiveGhosts());
      }
    });

    final source = _dartFmt.format(
      library.accept(dart.DartEmitter(_PrefixedAllocator())).toString(),
    );
    context.config.buildArtifactManager.setContent(
      context,
      moduleIdToBuildArtifactId(context, moduleId),
      source,
    );

    return Unit();
  },
);

final moduleIdToBuildArtifactId = Query<ModuleId, BuildArtifactId>(
  'dart.moduleIdToBuildArtifactId',
  evaluateAlways: true,
  provider: (context, moduleId) {
    return moduleId.packageId.dartBuildArtifactId
        .child(libDirectoryName)
        .child(srcDirectoryName)
        .child('${moduleId.path.join('/')}$dartFileExtension');
  },
);
final moduleIdToImportUrl = Query<ModuleId, String>(
  'dart.moduleIdToImportUrl',
  evaluateAlways: true,
  provider: (context, moduleId) {
    final packageName = moduleId.packageId.isThis
        ? getCandyspec(context, PackageId.this_).name
        : moduleId.packageId.name;
    return 'package:$packageName/$srcDirectoryName/${moduleId.path.join('/')}$dartFileExtension';
  },
);

/// Copy of `code_builder`'s _PrefixedAllocator that also prefixes core imports.
class _PrefixedAllocator implements dart.Allocator {
  final _imports = <String, int>{};
  var _keys = 1;

  @override
  String allocate(dart.Reference reference) {
    final symbol = reference.symbol;
    if (reference.url == null) {
      return symbol;
    }
    return '_i${_imports.putIfAbsent(reference.url, _nextKey)}.$symbol';
  }

  int _nextKey() => _keys++;

  @override
  Iterable<dart.Directive> get imports => _imports.keys.map(
        (u) => dart.Directive.import(u, as: '_i${_imports[u]}'),
      );
}
