import 'package:freezed_annotation/freezed_annotation.dart';

part 'data_classes.freezed.dart';

@freezed
abstract class Tuple2<T1, T2> with _$Tuple2<T1, T2> {
  const factory Tuple2(T1 first, T2 second) = _Tuple2;
}
