import 'dart:convert';

import 'package:compiler/compiler.dart';

import 'constants.dart';
import 'declarations/module.dart';
import 'pubspec.dart';

final compile = Query<Unit, Unit>(
  'dart.compile',
  evaluateAlways: true,
  provider: (context, _) {
    context.config.buildArtifactManager.delete(dartBuildArtifactId);

    final pubspec = generatePubspec(context, Unit());
    context.config.buildArtifactManager.setContent(
      dartBuildArtifactId.child(pubspecFile),
      jsonEncode(pubspec.toJson()),
    );

    compileModule(context, mainModuleId);

    return Unit();
  },
);
