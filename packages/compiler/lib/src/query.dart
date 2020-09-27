import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import 'resource_provider.dart';

typedef QueryProvider<K, R> = R Function(QueryContext context, K key);

class Query<K, R> {
  Query(
    this.name, {
    this.evaluateAlways = false,
    @required this.provider,
  })  : assert(name != null),
        assert(evaluateAlways != null),
        assert(provider != null);

  final String name;

  // Modifiers:
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
