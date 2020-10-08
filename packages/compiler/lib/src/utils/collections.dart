extension MapMapping<K, V> on Map<K, V> {
  Map<K, R> mapValues<R>(R Function(K key, V value) mapper) =>
      map((k, v) => MapEntry(k, mapper(k, v)));
}
