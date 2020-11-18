import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart' hide srcDirectoryName;
import 'package:dart_style/dart_style.dart';

import '../builtins.dart';
import '../constants.dart';
import '../utils.dart';
import 'declaration.dart';

final _dartFmt = DartFormatter();

final compileModule = Query<ModuleId, Unit>(
  'dart.compileModule',
  evaluateAlways: true,
  provider: (context, moduleId) {
    final module = getModuleDeclarationHir(context, moduleId);

    final library = dart.Library((b) {
      for (final declarationId in module.innerDeclarationIds) {
        b.body.addAll(compileDeclaration(context, declarationId));
      }

      if (moduleId == ModuleId.corePrimitives) {
        b.body.addAll(DartBuiltinCompiler(context).compilePrimitiveGhosts());
      }
    });

    final source = _dartFmt.format(
      library.accept(FancyDartEmitter(_PrefixedAllocator())).toString(),
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
final declarationIdToImportUrl = Query<DeclarationId, String>(
  'dart.declarationIdToImportUrl',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final moduleId = declarationIdToModuleId(context, declarationId);
    return moduleIdToImportUrl(context, moduleId);
  },
);
final moduleIdToImportUrl = Query<ModuleId, String>(
  'dart.moduleIdToImportUrl',
  evaluateAlways: true,
  provider: (context, moduleId) {
    return 'package:${moduleId.packageId.name}/$srcDirectoryName/${moduleId.path.join('/')}$dartFileExtension';
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
