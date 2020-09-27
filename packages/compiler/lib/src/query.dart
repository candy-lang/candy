import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import 'resource_provider.dart';

typedef QueryProvider<K, R> = R Function(QueryContext context, K key);

class Query<K, R> {
  Query(
    this.name, {
    bool persist = true,
    this.evaluateAlways = false,
    @required this.provider,
  })  : assert(name != null),
        assert(persist != null),
        assert(evaluateAlways != null),
        persist = persist && !evaluateAlways,
        assert(provider != null);

  final String name;

  // Modifiers:
  /// Results of this query won't be persisted.
  final bool persist;

  /// The result of this query isn't cached.
  ///
  /// This allows the query to read inputs (e.g., files).
  final bool evaluateAlways;

  final QueryProvider<K, R> provider;

  R call(QueryContext context, K key) {
    final result = provider(context, key);
    assert(result != null);
    return result;
  }
}

@immutable
class QueryContext {
  const QueryContext({
    @required this.resourceProvider,
  }) : assert(resourceProvider != null);

  final ResourceProvider resourceProvider;

  R callQuery<K, R>(Query<K, R> query, K key) => query(this, key);
}
