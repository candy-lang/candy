import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dart_style/dart_style.dart';
import 'package:path/path.dart' as p;

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
    });

    context.config.buildArtifactManager.setContent(
      moduleIdToBuildArtifactId(context, moduleId),
      _dartFmt.format(library.accept(dart.DartEmitter.scoped()).toString()),
    );

    return Unit();
  },
);

final moduleIdToBuildArtifactId = Query<ModuleId, BuildArtifactId>(
  'dart.moduleIdToBuildArtifactId',
  evaluateAlways: true,
  provider: (context, moduleId) {
    return dartBuildArtifactId
        .child(libDirectoryName)
        .child(srcDirectoryName)
        .child(moduleIdToPath(context, moduleId));
  },
);
final moduleIdToRelativePath = Query<Tuple2<ModuleId, ModuleId>, String>(
  'dart.moduleIdToPath',
  evaluateAlways: true,
  provider: (context, params) {
    final current = moduleIdToPath(context, params.first);
    final other = moduleIdToPath(context, params.second);

    return p.posix.relative('/$other', from: '/$current');
  },
);
final moduleIdToPath = Query<ModuleId, String>(
  'dart.moduleIdToPath',
  evaluateAlways: true,
  provider: (context, moduleId) {
    if (moduleId.packageId != PackageId.this_) {
      throw CompilerError.unsupportedFeature(
        'Compiling dependencies to Dart is not yet supported.',
      );
    }

    return '${moduleId.path.join('.')}$dartFileExtension';
  },
);
