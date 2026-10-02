import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import 'position.dart';

class const OptionConverter<T extends ToJson, J>(final Mapper<J, T> fromJsonT)
    implements JsonConverter<Option<T>, Map<String, dynamic>> {
  @override
  Option<T> fromJson(Map<String, dynamic> json) => switch (json['type']) {
    'some' => Some(fromJsonT(json['value'] as J)),
    'none' => const None(),
    final type => throw ArgumentError.value(type, 'type', 'Invalid `Option`'),
  };

  @override
  Map<String, dynamic> toJson(Option<T> object) => switch (object) {
    Some<T>(:final some) => {'type': 'some', 'value': some.toJson()},
    None<T>() => {'type': 'none'},
  };
}

class const OffsetRangeInclusiveConverter()
    implements JsonConverter<RangeInclusive<Offset>, Map<String, dynamic>> {
  @override
  RangeInclusive<Offset> fromJson(Map<String, dynamic> json) => RangeInclusive(
    .fromJson(json['start'] as Map<String, dynamic>),
    .fromJson(json['end'] as Map<String, dynamic>),
  );

  @override
  Map<String, dynamic> toJson(RangeInclusive<Offset> object) => {
    'start': object.start.toJson(),
    'end': object.end.toJson(),
  };
}
