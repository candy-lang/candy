import 'dart:io';

import 'package:compiler/compiler.dart';
import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:path/path.dart' as p;

import 'constants.dart';

part 'build_artifacts.freezed.dart';
part 'build_artifacts.g.dart';

@freezed
abstract class BuildArtifactId implements _$BuildArtifactId {
  const factory BuildArtifactId(PackageId packageId, String path) =
      _BuildArtifactId;
  factory BuildArtifactId.fromJson(Map<String, dynamic> json) =>
      _$BuildArtifactIdFromJson(json);
  const BuildArtifactId._();

  BuildArtifactId child(String name) => copyWith(path: '$path/$name');
}

class BuildArtifactManager {
  const BuildArtifactManager(this.projectDirectory);

  final Directory projectDirectory;

  void delete(QueryContext context, [BuildArtifactId directoryId]) {
    final path =
        toPath(context, directoryId ?? BuildArtifactId(PackageId.this_, ''));
    final dir = Directory(path);
    if (dir.existsSync()) dir.deleteSync(recursive: true);
  }

  bool fileExists(QueryContext context, BuildArtifactId id) =>
      File(toPath(context, id)).existsSync();

  String getContent(QueryContext context, BuildArtifactId id) =>
      File(toPath(context, id)).readAsStringSync();
  void setContent(QueryContext context, BuildArtifactId id, String content) {
    File(toPath(context, id))
      ..createSync(recursive: true)
      ..writeAsStringSync(content);
  }

  String toPath(QueryContext context, BuildArtifactId id) {
    final packageDirectory = context.config.resourceProvider
        .getPackageDirectory(context, id.packageId);
    return p.join(packageDirectory.path, buildDirectoryName, id.path);
  }
}
