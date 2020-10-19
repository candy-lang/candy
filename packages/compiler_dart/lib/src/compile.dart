import 'dart:convert';
import 'dart:io';

import 'package:compiler/compiler.dart';
import 'package:path/path.dart' as p;
import 'package:pubspec/pubspec.dart';

import 'constants.dart';
import 'declarations/module.dart';
import 'pubspec.dart';

final compile = Query<PackageId, Unit>(
  'dart.compile',
  evaluateAlways: true,
  provider: (context, packageId) {
    if (packageId.isThis) {
      // TODO(JonasWanke): compile transitive dependencies when they're supported
      compile(context, PackageId.core);

      final dependencies = getCandyspec(context, packageId).dependencies;
      for (final dependency in dependencies.keys) {
        compile(context, PackageId(dependency));
      }
    }

    final buildArtifactId = packageId.dartBuildArtifactId;
    context.config.buildArtifactManager.delete(context, buildArtifactId);

    final pubspec = generatePubspec(context, packageId);
    final ioSink = _SimpleIoSink();
    YamlToString().writeYamlString(pubspec.toJson(), ioSink);
    context.config.buildArtifactManager.setContent(
      context,
      buildArtifactId.child(pubspecFile),
      ioSink.toString(),
    );

    final allResourceIds = context.config.resourceProvider
        .getAllFileResourceIds(context, packageId)
        .where((id) => id.isCandySourceFile);
    for (final resourceId in allResourceIds) {
      compileModule(context, resourceIdToModuleId(context, resourceId));
    }

    runPubGet(context, packageId);

    return Unit();
  },
);

final runPubGet = Query<PackageId, Unit>(
  'dart.runPubGet',
  evaluateAlways: true,
  provider: (context, packageId) {
    final result = Process.runSync(
      'pub.bat',
      ['get'],
      workingDirectory: context.config.buildArtifactManager
          .toPath(context, packageId.dartBuildArtifactId),
    );
    if (result.exitCode != 0) {
      throw CompilerError.internalError(
        'Error running `pub get`:\n${result.stdout}\n${result.stderr}',
      );
    }

    return Unit();
  },
);
final run = Query<PackageId, String>(
  'dart.run',
  evaluateAlways: true,
  provider: (context, packageId) {
    final buildArtifactId = packageId.dartBuildArtifactId;
    final directory =
        context.config.buildArtifactManager.toPath(context, buildArtifactId);
    final path = p.relative(
      context.config.buildArtifactManager
          .toPath(context, moduleIdToBuildArtifactId(context, mainModuleId)),
      from:
          context.config.buildArtifactManager.toPath(context, buildArtifactId),
    );
    final result = Process.runSync('dart', [path], workingDirectory: directory);

    return [
      'Exit Code: ${result.exitCode}',
      if ((result.stdout as String).isNotEmpty) 'stdout:\n${result.stdout}',
      if ((result.stderr as String).isNotEmpty) 'stderr:\n${result.stderr}',
    ].join('\n');
  },
);

class _SimpleIoSink implements IOSink {
  _SimpleIoSink() : _buffer = StringBuffer();

  final StringBuffer _buffer;
  @override
  Encoding encoding = utf8;

  @override
  void add(List<int> data) {
    throw UnimplementedError();
  }

  @override
  void addError(Object error, [StackTrace stackTrace]) {
    throw UnimplementedError();
  }

  @override
  Future<void> addStream(Stream<List<int>> stream) {
    throw UnimplementedError();
  }

  @override
  Future<void> close() async {}
  @override
  Future<void> get done => Future.value();
  @override
  Future<void> flush() => Future.value();

  @override
  void write(Object obj) => _buffer.write(obj);
  @override
  void writeAll(Iterable<Object> objects, [String separator = '']) =>
      _buffer.writeAll(objects, separator);
  @override
  void writeCharCode(int charCode) => _buffer.writeCharCode(charCode);
  @override
  void writeln([Object obj = '']) => _buffer.writeln(obj);

  @override
  String toString() => _buffer.toString();
}
