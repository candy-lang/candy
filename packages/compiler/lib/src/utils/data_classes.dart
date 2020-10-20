import 'package:freezed_annotation/freezed_annotation.dart';

part 'data_classes.freezed.dart';

@freezed
abstract class Unit with _$Unit {
  const factory Unit() = _Unit;
}

@freezed
abstract class Option<T> implements _$Option<T> {
  const factory Option.some(T value) = Some<T>;
  const factory Option.none() = None<T>;
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

  Option<R> mapValue<R>(R Function(T) mapper) =>
      flatMapValue((value) => Some(mapper(value)));
  Option<R> flatMapValue<R>(Option<R> Function(T) mapper) => when(
        some: (value) => mapper(value),
        none: () => None(),
      );

  List<T> toList() => when(some: (value) => [value], none: () => []);
}

@freezed
abstract class Result<T, E> implements _$Result<T, E> {
  const factory Result.ok(T value) = Ok<T, E>;
  const factory Result.error(E error) = Error<T, E>;
  const Result._();

  T get value => when(
        ok: (value) => value,
        error: (_) {
          assert(false);
          return null;
        },
      );
  E get error => when(
        ok: (_) {
          assert(false);
          return null;
        },
        error: (error) => error,
      );

  Result<R, E> mapValue<R>(R Function(T) mapper) => when(
        ok: (value) => Ok(mapper(value)),
        error: (error) => Error(error),
      );
}

@freezed
abstract class Tuple2<T1, T2> with _$Tuple2<T1, T2> {
  const factory Tuple2(T1 first, T2 second) = _Tuple2<T1, T2>;
}

@freezed
abstract class Tuple3<T1, T2, T3> with _$Tuple3<T1, T2, T3> {
  const factory Tuple3(T1 first, T2 second, T3 third) = _Tuple3<T1, T2, T3>;
}

@freezed
abstract class Tuple4<T1, T2, T3, T4> with _$Tuple4<T1, T2, T3, T4> {
  const factory Tuple4(T1 first, T2 second, T3 third, T4 fourth) =
      _Tuple4<T1, T2, T3, T4>;
}
