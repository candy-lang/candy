import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import 'utils.dart';

part 'source_span.freezed.dart';
part 'source_span.g.dart';

@freezed
abstract class SourceSpan implements _$SourceSpan {
  const factory SourceSpan(int start, int end) = _SourceSpan;
  factory SourceSpan.fromJson(Map<String, dynamic> json) =>
      _$SourceSpanFromJson(json);
  const SourceSpan._();

  // ignore: prefer_constructors_over_static_methods
  static SourceSpan fromStartLength(int start, int length) =>
      SourceSpan(start, start + length);

  int get length => end - start;

  SourceSpan plus(int offset) {
    assert(offset != null);

    return SourceSpan(start + offset, end + offset);
  }

  @override
  String toString() => '$start–$end';

  @override
  bool operator ==(Object other) =>
      other is SourceSpan && start == other.start && end == other.end;

  @override
  int get hashCode => hashList([start, end]);
}
