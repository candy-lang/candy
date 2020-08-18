import 'package:meta/meta.dart';

import 'utils.dart';

@immutable
class SourceSpan {
  const SourceSpan(this.start, this.end)
      : assert(start != null),
        assert(start >= 0),
        assert(end != null),
        assert(start <= end);

  final int start;
  final int end;
  int get length => end - start;

  @override
  String toString() => '$start–$end';

  @override
  bool operator ==(Object other) =>
      other is SourceSpan && start == other.start && end == other.end;

  @override
  int get hashCode => hashList([start, end]);
}
