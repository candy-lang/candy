import 'dart:io';

import 'package:supernova/supernova.dart';

import 'position.dart';

@immutable
class const SourceFile(final File file, final String content) {
  factory fromJson(String json) => SourceFile(File(json), '');

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

  /// The inverse of [locationOf]. A line or column past the end of what's
  /// there is clamped, since an editor's cursor can outrun our copy of the
  /// file by a keystroke.
  Offset offsetOf({required int line, required int column}) {
    var offset = 0;
    for (var current = 1; current < line; current++) {
      final lineBreak = content.indexOf('\n', offset);
      if (lineBreak < 0) return Offset(content.length);
      offset = lineBreak + 1;
    }

    final lineBreak = content.indexOf('\n', offset);
    final lineEnd = lineBreak < 0 ? content.length : lineBreak;
    return Offset((offset + column - 1).clamp(offset, lineEnd));
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
