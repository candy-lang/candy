import 'dart:io';

import 'package:supernova/supernova.dart';

@immutable
class SourceFile {
  const SourceFile(this.file, this.content);

  factory SourceFile.fromJson(String json) => SourceFile(File(json), '');

  final File file;
  final String content;

  String toJson() => file.path;

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        other is SourceFile &&
            runtimeType == other.runtimeType &&
            file.path == other.file.path &&
            content == other.content;
  }

  @override
  int get hashCode => file.path.hashCode ^ content.hashCode;

  @override
  String toString() => file.path;
}
