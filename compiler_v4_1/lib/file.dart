import 'dart:io';

import 'package:supernova/supernova.dart';

import 'position.dart';

@immutable
class SourceFile {
  const SourceFile(this.file, this.content);

  factory SourceFile.fromJson(String json) => SourceFile(File(json), '');

  final File file;
  final String content;

  String toJson() => file.path;

  /// The 1-based line and column of [offset], counting UTF-16 code units
  /// like [offset] itself so this stays LSP-compatible.
  ({int line, int column}) locationOf(Offset offset) {
    final index = offset.value.clamp(0, content.length);
    final before = content.substring(0, index);
    final line = '\n'.allMatches(before).length + 1;
    final lastNewline = before.lastIndexOf('\n');
    return (line: line, column: index - lastNewline);
  }

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
