import 'dart:io';

import 'package:supernova/supernova.dart' hide IterableSorted;

import 'position.dart';

/// Where a name in the source points, recorded while lowering resolves it.
class const Reference({
  required final SourceSpan span,

  /// More than one when a call couldn't be narrowed down to a single overload.
  /// Editors offer the user a choice between them.
  required final List<SourceSpan> targets,
});

/// Every resolved name in the program, so the language server can answer
/// "what does this point at?" and "what points at this?" without redoing name
/// resolution.
class ReferenceIndex {
  final _byFile = <String, Map<int, Reference>>{};

  /// Every name the program declares, keyed by file and start offset. Kept
  /// separately from [_byFile] so that a declaration nothing uses is still
  /// something find-references can start from.
  final _declarationsByFile = <String, Map<int, SourceSpan>>{};

  /// The reverse of [_byFile], built on demand. Lowering records everything
  /// before anything asks, so rebuilding lazily is cheaper than keeping the two
  /// in step while a call is still being retried.
  Map<(String, int, int), _Definition>? _byTarget;

  /// Lowering resolves the same name repeatedly when a call is retried after
  /// more types become known, so the last recording for a span wins.
  void add(SourceSpan span, Iterable<SourceSpan> targets) {
    final list = targets.toList();
    if (list.isEmpty) return;

    _byFile.putIfAbsent(
      _pathOf(span.file.file),
      () => {},
    )[span.span.start.value] = Reference(
      span: span,
      targets: list,
    );
    _byTarget = null;
  }

  void addDeclaration(SourceSpan span) => _declarationsByFile.putIfAbsent(
    _pathOf(span.file.file),
    () => {},
  )[span.span.start.value] = span;

  /// What the name at [offset] points at, empty if there's no name there.
  List<SourceSpan> targetsAt(File file, Offset offset) {
    final references = _byFile[_pathOf(file)];
    if (references == null) return const [];

    for (final reference in references.values) {
      if (_contains(reference.span, offset)) return reference.targets;
    }
    return const [];
  }

  /// Every place that points at whatever is at [offset], which may be either a
  /// name pointing somewhere or the declaration being pointed at.
  ///
  /// Only covers what was analyzed: a package that nothing open depends on
  /// contributes no references, however many it has.
  List<SourceSpan> referencesAt(
    File file,
    Offset offset, {
    required bool includeDeclaration,
  }) {
    final definitions = targetsAt(file, offset).toList();
    if (definitions.isEmpty) {
      final declaration = _declarationAt(file, offset);
      if (declaration == null) return const [];
      definitions.add(declaration);
    }

    final found = <(String, int, int), SourceSpan>{};
    for (final definition in definitions) {
      if (includeDeclaration) found[_keyOf(definition)] = definition;
      for (final reference
          in _definitions[_keyOf(definition)]?.references ??
              const <SourceSpan>[]) {
        found[_keyOf(reference)] = reference;
      }
    }
    return found.values.sorted(_bySourceOrder);
  }

  /// Falls back to what references point at, which covers the few targets
  /// lowering resolves to without the collector having declared them — an
  /// impl's `Self`, above all.
  SourceSpan? _declarationAt(File file, Offset offset) {
    final path = _pathOf(file);
    for (final span
        in _declarationsByFile[path]?.values ?? const <SourceSpan>[]) {
      if (_contains(span, offset)) return span;
    }

    for (final definition in _definitions.values) {
      if (_pathOf(definition.span.file.file) != path) continue;
      if (_contains(definition.span, offset)) return definition.span;
    }
    return null;
  }

  Map<(String, int, int), _Definition> get _definitions {
    final built = _byTarget;
    if (built != null) return built;

    final definitions = <(String, int, int), _Definition>{};
    for (final file in _byFile.values) {
      for (final reference in file.values) {
        for (final target in reference.targets) {
          definitions
              .putIfAbsent(_keyOf(target), () => _Definition(target))
              .references
              .add(reference.span);
        }
      }
    }
    return _byTarget = definitions;
  }

  /// The end offset counts as part of the span, so that a cursor sitting
  /// directly behind an identifier still resolves it, like every editor
  /// expects.
  static bool _contains(SourceSpan span, Offset offset) =>
      offset.value >= span.span.start.value &&
      offset.value <= span.span.end.value;

  static int _bySourceOrder(SourceSpan a, SourceSpan b) {
    final byFile = _pathOf(a.file.file).compareTo(_pathOf(b.file.file));
    return byFile != 0
        ? byFile
        : a.span.start.value.compareTo(b.span.start.value);
  }

  static (String, int, int) _keyOf(SourceSpan span) =>
      (_pathOf(span.file.file), span.span.start.value, span.span.end.value);

  static String _pathOf(File file) => file.absolute.path;
}

class _Definition {
  new(this.span);

  final SourceSpan span;
  final references = <SourceSpan>[];
}
