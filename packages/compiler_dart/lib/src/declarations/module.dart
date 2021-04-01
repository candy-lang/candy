import 'dart:io';

import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart' hide srcDirectoryName;
import 'package:dart_style/dart_style.dart';

import '../body.dart';
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
      if (moduleId == ModuleId.coreString) {
        b.directives.add(dart.Directive((b) => b
          ..type = dart.DirectiveType.import
          ..url = 'package:characters/characters.dart'));
      }
    });

    if (library.body.isNotEmpty) {
      final rawSource =
          library.accept(FancyDartEmitter(_PrefixedAllocator())).toString();
      String source;
      try {
        source = _dartFmt.format(rawSource);
      } on FormatterException {
        stderr.write('Syntax error in generated source of module $moduleId.');
        context.config.buildArtifactManager.setContent(
          context,
          moduleIdToBuildArtifactId(context, moduleId),
          rawSource,
        );
        rethrow;
      }
      context.config.buildArtifactManager.setContent(
        context,
        moduleIdToBuildArtifactId(context, moduleId),
        source,
      );
    }

    compileModuleTests(context, moduleId);

    return Unit();
  },
);
final compileModuleTests = Query<ModuleId, Unit>(
  'dart.compileModuleTests',
  evaluateAlways: true,
  provider: (context, moduleId) {
    final module = getModuleDeclarationHir(context, moduleId);

    final testFunctions = module.innerDeclarationIds
        .where((it) => it.isFunction)
        .where((it) => getFunctionDeclarationHir(context, it).isTest)
        .map((it) => Tuple2(it, compileBody(context, it)))
        .where((it) => it.second is Some)
        .map((it) {
      final id = it.first;
      final code = it.second.value;

      return dart.refer('test', packageTestUrl).call(
        [
          dart.literalString(id.simplePath.last.nameOrNull),
          dart.Method((b) => b.body = code).closure,
        ],
        {},
        [],
      ).statement;
    });
    if (testFunctions.isNotEmpty) {
      final mainFunction = dart.Method((b) => b
        ..name = 'main'
        ..body = dart.Block((b) => b.statements.addAll(testFunctions)));
      final library = dart.Library((b) => b..body.add(mainFunction));
      final source = _dartFmt.format(
        library.accept(FancyDartEmitter(_PrefixedAllocator())).toString(),
      );
      context.config.buildArtifactManager.setContent(
        context,
        moduleIdToTestBuildArtifactId(context, moduleId),
        source,
      );
    }

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
final moduleIdToTestBuildArtifactId = Query<ModuleId, BuildArtifactId>(
  'dart.moduleIdToTestBuildArtifactId',
  evaluateAlways: true,
  provider: (context, moduleId) {
    return moduleId.packageId.dartBuildArtifactId
        .child(libDirectoryName)
        .child(testDirectoryName)
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
