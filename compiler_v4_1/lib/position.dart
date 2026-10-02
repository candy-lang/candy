import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import 'file.dart';
import 'json_converters.dart';

part 'position.freezed.dart';
part 'position.g.dart';

/// The offset of a character in a string as the number of UTF-16 code units
/// preceding it.
@freezed
sealed class const Offset._() with _$Offset implements Comparable<Offset> {
  const factory(int value) = _Offset;

  factory fromJson(Map<String, dynamic> json) => _$OffsetFromJson(json);

  @override
  int compareTo(Offset other) => value.compareTo(other.value);

  Offset operator +(int other) => Offset(value + other);
  Offset operator -(int other) => Offset(value - other);

  @override
  String toString() => value.toString();
}

@freezed
sealed class const SourceSpan._() with _$SourceSpan {
  const factory(
    SourceFile file,
    @OffsetRangeInclusiveConverter() RangeInclusive<Offset> span,
  ) = _SourceSpan;

  factory fromJson(Map<String, dynamic> json) => _$SourceSpanFromJson(json);

  /// `file:line:column`, e.g. for pointing at a single location.
  String get startLocation {
    final start = file.locationOf(span.start);
    return '$file:${start.line}:${start.column}';
  }

  @override
  String toString() {
    final start = file.locationOf(span.start);
    final end = file.locationOf(span.end);
    return '$file:${start.line}:${start.column}-${end.line}:${end.column}';
  }
}
