import 'dart:io';

import 'position.dart';

/// Where a name in the source points, recorded while lowering resolves it.
class Reference {
  const Reference({required this.span, required this.targets});

  final SourceSpan span;

  /// More than one when a call couldn't be narrowed down to a single overload.
  /// Editors offer the user a choice between them.
  final List<SourceSpan> targets;
}

/// Every resolved name in the program, so the language server can answer
/// "what does this point at?" without redoing name resolution.
class ReferenceIndex {
  final _byFile = <String, Map<int, Reference>>{};

  /// Lowering resolves the same name repeatedly when a call is retried after
  /// more types become known, so the last recording for a span wins.
  void add(SourceSpan span, Iterable<SourceSpan> targets) {
    final list = targets.toList();
    if (list.isEmpty) return;

    _byFile.putIfAbsent(
      _keyOf(span.file.file),
      () => {},
    )[span.span.start.value] = Reference(
      span: span,
      targets: list,
    );
  }

  /// The end offset is treated as part of the span so that a cursor sitting
  /// directly behind an identifier still resolves it, like every editor
  /// expects.
  List<SourceSpan> targetsAt(File file, Offset offset) {
    final references = _byFile[_keyOf(file)];
    if (references == null) return const [];

    for (final reference in references.values) {
      final span = reference.span.span;
      if (offset.value >= span.start.value && offset.value <= span.end.value) {
        return reference.targets;
      }
    }
    return const [];
  }

  static String _keyOf(File file) => file.absolute.path;
}
