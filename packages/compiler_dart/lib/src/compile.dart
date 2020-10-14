import 'dart:convert';
import 'dart:io';

import 'package:compiler/compiler.dart';
import 'package:path/path.dart' as p;
import 'package:pubspec/pubspec.dart';

import 'constants.dart';
import 'declarations/module.dart';
import 'pubspec.dart';

final compile = Query<Unit, Unit>(
  'dart.compile',
  evaluateAlways: true,
  provider: (context, _) {
    context.config.buildArtifactManager.delete(dartBuildArtifactId);

    final pubspec = generatePubspec(context, Unit());
    final ioSink = _SimpleIoSink();
    YamlToString().writeYamlString(pubspec.toJson(), ioSink);
    context.config.buildArtifactManager
        .setContent(dartBuildArtifactId.child(pubspecFile), ioSink.toString());

    compileModule(context, mainModuleId);

    runPubGet(context, Unit());

    return Unit();
  },
);

final runPubGet = Query<Unit, Unit>(
  'dart.runPubGet',
  evaluateAlways: true,
  provider: (context, _) {
    final result = Process.runSync(
      'pub.bat',
      ['get'],
      workingDirectory:
          context.config.buildArtifactManager.toPath(dartBuildArtifactId),
    );
    if (result.exitCode != 0) {
      throw CompilerError.internalError(
        'Error running `pub get`:\n${result.stdout}\n${result.stderr}',
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
