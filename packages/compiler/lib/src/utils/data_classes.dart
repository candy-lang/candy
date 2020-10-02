import 'package:freezed_annotation/freezed_annotation.dart';

part 'data_classes.freezed.dart';

@freezed
abstract class Unit with _$Unit {
  const factory Unit() = _Unit;
}

@freezed
abstract class Option<T> implements _$Option<T> {
  const factory Option.some(T value) = OptionSome<T>;
  const factory Option.none() = OptionNone<T>;
  const Option._();

  bool get isSome => when(some: (_) => true, none: () => false);
  bool get isNone => !isSome;

  T get value {
    return when(
      some: (value) => value,
      none: () {
        assert(false);
        return null;
      },
    );
  }

  T get valueOrNull => when(some: (value) => value, none: () => null);
}

@freezed
abstract class Tuple2<T1, T2> with _$Tuple2<T1, T2> {
  const factory Tuple2(T1 first, T2 second) = _Tuple2<T1, T2>;
}
