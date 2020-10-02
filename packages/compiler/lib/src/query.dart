import 'package:dartx/dartx.dart';
import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import 'build_artifacts.dart';
import 'compilation/ast.dart';
import 'errors.dart';
import 'resource_provider.dart';
import 'utils.dart';

@immutable
class QueryConfig {
  const QueryConfig({
    @required this.resourceProvider,
    @required this.buildArtifactManager,
  })  : assert(resourceProvider != null),
        assert(buildArtifactManager != null);

  final ResourceProvider resourceProvider;
  final BuildArtifactManager buildArtifactManager;

  // ignore: use_to_and_as_if_applicable
  GlobalQueryContext createContext() => GlobalQueryContext(this);
}

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
class GlobalQueryContext {
  GlobalQueryContext(this.config) : assert(config != null);

  final QueryConfig config;

  Option<R> callQuery<K, R>(Query<K, R> query, K key) {
    final innerContext = QueryContext(this);
    try {
      return Option.some(innerContext.callQuery(query, key));
    } on _QueryFailedException {
      return Option.none();
    }
  }

  final Map<Tuple2<String, dynamic>, List<ReportedCompilerError>>
      _reportedErrors = {};
  Iterable<ReportedCompilerError> get reportedErrors =>
      _reportedErrors.values.flatten();
  Map<ResourceId, List<ReportedCompilerError>> get reportedErrorsByResourceId =>
      reportedErrors.groupBy((e) => e.location?.resourceId);
  void _reportErrors(
    String queryName,
    Object key,
    List<ReportedCompilerError> errors,
  ) {
    if (errors.isNotEmpty) {
      _reportedErrors[Tuple2(queryName, key)] = errors;
    } else {
      _reportedErrors.remove(Tuple2(queryName, key));
    }
  }
}

class QueryContext {
  QueryContext(this.globalContext) : assert(globalContext != null);

  final GlobalQueryContext globalContext;
  QueryConfig get config => globalContext.config;

  R callQuery<K, R>(Query<K, R> query, K key) =>
      QueryContext(globalContext)._execute(query, key);
  R _execute<K, R>(Query<K, R> query, K key) {
    void reportErrors() =>
        globalContext._reportErrors(query.name, key, _reportedErrors);

    try {
      final result = query(this, key);
      reportErrors();
      return result;
    } on ReportedCompilerError catch (e) {
      _reportedErrors.add(e);
      reportErrors();
      throw _QueryFailedException();
    } catch (e, st) {
      _reportedErrors.add(CompilerError.internalError('$e\n\n$st'));
      reportErrors();
      throw _QueryFailedException();
    }
  }

  final _reportedErrors = <ReportedCompilerError>[];
  void reportError(ReportedCompilerError error) => _reportedErrors.add(error);
}

class _QueryFailedException implements Exception {}
