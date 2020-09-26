import 'package:freezed_annotation/freezed_annotation.dart';

import 'ids.dart';

part 'type.freezed.dart';
part 'type.g.dart';

@freezed
abstract class CandyType with _$CandyType {
  const factory CandyType.user(
    ModuleId moduleId,
    String name, {
    @Default(<CandyType>[]) List<CandyType> arguments,
  }) = UserCandyType;
  const factory CandyType.tuple(List<CandyType> items) = TupleCandyType;
  const factory CandyType.function({
    CandyType receiverType,
    @Default(<CandyType>[]) List<CandyType> parameterTypes,
    CandyType returnType,
  }) = FunctionCandyType;
  const factory CandyType.union(List<CandyType> types) = UnionCandyType;
  const factory CandyType.intersection(List<CandyType> types) =
      IntersectionCandyType;

  factory CandyType.fromJson(Map<String, dynamic> json) =>
      _$CandyTypeFromJson(json);
  const CandyType._();

  static const any = CandyType.user(ModuleId.corePrimitives, 'Any');
  static const unit = CandyType.user(ModuleId.corePrimitives, 'Unit');
  static const nothing = CandyType.user(ModuleId.corePrimitives, 'Nothing');

  static const bool = CandyType.user(ModuleId.corePrimitives, 'Bool');
  static const number = CandyType.user(ModuleId.corePrimitives, 'Number');
  static const int = CandyType.user(ModuleId.corePrimitives, 'Int');
  static const float = CandyType.user(ModuleId.corePrimitives, 'Float');
  static const string = CandyType.user(ModuleId.corePrimitives, 'String');
}
