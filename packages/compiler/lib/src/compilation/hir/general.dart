import 'package:freezed_annotation/freezed_annotation.dart';

import 'ids.dart';

part 'general.freezed.dart';
part 'general.g.dart';

@freezed
abstract class UseLine implements _$UseLine {
  const factory UseLine(
    ModuleId moduleId, {
    @required bool isPublic,
  }) = _UseLine;
  factory UseLine.fromJson(Map<String, dynamic> json) =>
      _$UseLineFromJson(json);
  const UseLine._();
}
