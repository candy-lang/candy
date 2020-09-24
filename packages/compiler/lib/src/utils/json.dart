typedef JsonDeserializer<T> = T Function(dynamic json);

JsonDeserializer<T> defaultJsonDeserializer<T>() {
  if (T == String) return (dynamic json) => json as T;

  assert(
    false,
    "Type $T can't automatically be deserialized from JSON.",
  );
  return null;
}

dynamic toJson(dynamic value) {
  if (value is String) return value;
  return value.toJson();
}
