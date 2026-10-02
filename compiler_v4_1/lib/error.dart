import 'package:supernova/supernova.dart';

import 'position.dart';

part 'error.freezed.dart';

@freezed
sealed class const CompilerError._()
    with _$CompilerError
    implements Comparable<CompilerError> {
  const factory({required SourceSpan span, required String message}) =
      _CompilerError;

  /// Orders errors by where they are, so reporting doesn't depend on the order
  /// the phases happened to produce them in.
  @override
  int compareTo(CompilerError other) {
    final byFile = span.file.file.path.compareTo(other.span.file.file.path);
    if (byFile != 0) return byFile;
    final byStart = span.span.start.compareTo(other.span.span.start);
    if (byStart != 0) return byStart;
    final byEnd = span.span.end.compareTo(other.span.span.end);
    if (byEnd != 0) return byEnd;
    return message.compareTo(other.message);
  }
}

/// The errors of several analyses as one ordered list, each error only once.
///
/// A file that two analyzed packages both reach is analyzed once per package,
/// so its errors arrive once per package. Comparing where an error is rather
/// than the [CompilerError] itself keeps that independent of how each analysis
/// read the file, so an editor's unsaved buffer doesn't count as a second file.
List<CompilerError> mergeErrors(Iterable<Iterable<CompilerError>> analyses) {
  final seen = <(String, int, int, String)>{};
  final errors = <CompilerError>[];
  for (final analysis in analyses) {
    for (final error in analysis) {
      final key = (
        error.span.file.file.absolute.path,
        error.span.span.start.value,
        error.span.span.end.value,
        error.message,
      );
      if (seen.add(key)) errors.add(error);
    }
  }
  return errors.sorted();
}
