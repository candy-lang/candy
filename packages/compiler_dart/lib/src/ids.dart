import 'package:compiler/compiler.dart';
import 'package:path/path.dart' as p;

import 'constants.dart';

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
