import 'dart:convert';
import 'dart:io';

import 'package:compiler/compiler.dart';
import 'package:path/path.dart' as p;

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

final runPubGet = Query<Unit, Unit>(
  'dart.runPubGet',
  evaluateAlways: true,
  provider: (context, _) {
    final result = Process.runSync(
      'pub',
      ['get'],
      workingDirectory:
          context.config.buildArtifactManager.toPath(dartBuildArtifactId),
    );
    if (result.exitCode != 0) {
      throw CompilerError.internalError(
        'Error running `pub get`: ${result.stdout}',
      );
    }

    return Unit();
  },
);
final run = Query<Unit, String>(
  'dart.run',
  evaluateAlways: true,
  provider: (context, _) {
    final directory =
        context.config.buildArtifactManager.toPath(dartBuildArtifactId);
    final path = p.relative(
      context.config.buildArtifactManager
          .toPath(moduleIdToBuildArtifactId(context, mainModuleId)),
      from: context.config.buildArtifactManager.toPath(dartBuildArtifactId),
    );
    final result = Process.runSync('dart', [path], workingDirectory: directory);

    return [
      'Exit Code: ${result.exitCode}',
      if ((result.stdout as String).isNotEmpty) 'stdout:\n${result.stdout}',
      if ((result.stderr as String).isNotEmpty) 'stderr:\n${result.stderr}',
    ].join('\n');
  },
);
