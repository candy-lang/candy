import 'package:compiler/compiler.dart';

import '../ids.dart';

final compileModule = Query<ModuleId, Unit>(
  'dart.compileModule',
  evaluateAlways: true,
  provider: (context, moduleId) {
    final relativePath =
        moduleIdToRelativePath(context, Tuple2(moduleId, mainModuleId));
    context.config.buildArtifactManager.setContent(
      moduleIdToBuildArtifactId(context, moduleId),
      '// $relativePath',
    );

    return Unit();
  },
);
