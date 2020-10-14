import 'dart:io';

import 'package:compiler/compiler.dart';
import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:path/path.dart' as p;

import 'constants.dart';

part 'build_artifacts.freezed.dart';
part 'build_artifacts.g.dart';

@freezed
abstract class BuildArtifactId implements _$BuildArtifactId {
  const factory BuildArtifactId(String path) = _BuildArtifactId;
  factory BuildArtifactId.fromJson(Map<String, dynamic> json) =>
      _$BuildArtifactIdFromJson(json);
  const BuildArtifactId._();

  BuildArtifactId child(String name) => BuildArtifactId('$path/$name');
}

class BuildArtifactManager {
  const BuildArtifactManager(this.projectDirectory);

  final Directory projectDirectory;
  String get _buildDirectory =>
      p.join(projectDirectory.path, buildDirectoryName);

  void delete([BuildArtifactId directoryId]) {
    final path = directoryId != null ? toPath(directoryId) : _buildDirectory;
    final dir = Directory(path);
    if (dir.existsSync()) dir.deleteSync(recursive: true);
  }

  bool fileExists(BuildArtifactId id) => File(toPath(id)).existsSync();

  String getContent(BuildArtifactId id) => File(toPath(id)).readAsStringSync();
  void setContent(BuildArtifactId id, String content) {
    File(toPath(id))
      ..createSync(recursive: true)
      ..writeAsStringSync(content);
  }

  String toPath(BuildArtifactId id) => p.join(_buildDirectory, id.path);
}
