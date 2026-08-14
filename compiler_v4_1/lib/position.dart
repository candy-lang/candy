import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import 'file.dart';
import 'json_converters.dart';

part 'position.freezed.dart';
part 'position.g.dart';

/// The offset of a character in a string as the number of UTF-16 code units
/// preceding it.
@freezed
sealed class Offset with _$Offset implements Comparable<Offset> {
  const factory Offset(int value) = _Offset;

  factory Offset.fromJson(Map<String, dynamic> json) => _$OffsetFromJson(json);
  const Offset._();

  @override
  int compareTo(Offset other) => value.compareTo(other.value);

  Offset operator +(int other) => Offset(value + other);
  Offset operator -(int other) => Offset(value - other);

  @override
  String toString() => value.toString();
}

@freezed
sealed class SourceSpan with _$SourceSpan {
  const factory SourceSpan(
    SourceFile file,
    @OffsetRangeInclusiveConverter() RangeInclusive<Offset> span,
  ) = _SourceSpan;

  factory SourceSpan.fromJson(Map<String, dynamic> json) =>
      _$SourceSpanFromJson(json);
  const SourceSpan._();

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
