import 'dart:convert';

import 'package:dartx/dartx.dart';
import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:meta/meta.dart';

import 'build_artifacts.dart';
import 'compilation/ast.dart';
import 'errors.dart';
import 'resource_provider.dart';
import 'utils.dart';

part 'query.freezed.dart';
part 'query.g.dart';

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
    final result = context.callQuery(this, key);
    assert(result != null);
    return result;
  }

  R execute(QueryContext context, K key) {
    final result = Timeline.timeSync(
      '$name($key)',
      () => provider(context, key),
    );
    assert(result != null);
    return result;
  }
}

@immutable
class GlobalQueryContext {
  GlobalQueryContext(this.config) : assert(config != null);

  final QueryConfig config;

  Option<R> callQuery<K, R>(Query<K, R> query, K key) {
    RecordedQueryCall result;
    try {
      result = QueryContext(this)._execute(query, key);
    } on _QueryFailedException catch (e) {
      result = e.recordedCall;
    }

    if (query.name.startsWith('dart.') || query.name == 'getAst') {
      var dateTime = DateTime.now().toIso8601String();
      dateTime =
          dateTime.substring(0, dateTime.indexOf('.')).replaceAll(':', '-');
      final encoder =
          JsonEncoder.withIndent('  ', (object) => object.toString());
      config.buildArtifactManager.setContent(
        BuildArtifactId('query-traces/$dateTime ${query.name}.json'),
        encoder.convert(result.toJson()),
      );
    }

    return result.result != null ? Some(result.result as R) : None();
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

  R callQuery<K, R>(Query<K, R> query, K key) {
    final result = QueryContext(globalContext)._execute(query, key);
    _innerCalls.add(result);
    if (result.result == null) {
      throw _QueryFailedException(result);
    }
    return result.result as R;
  }

  RecordedQueryCall _execute<K, R>(Query<K, R> query, K key) {
    void reportErrors() =>
        globalContext._reportErrors(query.name, key, _reportedErrors);
    RecordedQueryCall onErrors(dynamic error) {
      reportErrors();
      return RecordedQueryCall(
        name: query.name,
        key: key,
        innerCalls: _innerCalls,
        thrownErrors: error is _QueryFailedException
            ? error.recordedCall.thrownErrors
            : error is Iterable<ReportedCompilerError>
                ? error.toList()
                : [error as ReportedCompilerError],
      );
    }

    try {
      final result = query.execute(this, key);
      reportErrors();
      return RecordedQueryCall(
        name: query.name,
        key: key,
        innerCalls: _innerCalls,
        result: result,
      );
    } on ReportedCompilerError catch (e) {
      return onErrors(e);
    } on Iterable<ReportedCompilerError> catch (e) {
      return onErrors(e);
    } catch (e, st) {
      return onErrors(e is _QueryFailedException
          ? e
          : CompilerError.internalError('$e\n\n$st'));
    }
  }

  final _reportedErrors = <ReportedCompilerError>[];
  void reportError(ReportedCompilerError error) => _reportedErrors.add(error);

  final _innerCalls = <RecordedQueryCall>[];
}

@freezed
abstract class _QueryFailedException
    implements _$_QueryFailedException, Exception {
  const factory _QueryFailedException(RecordedQueryCall recordedCall) =
      __QueryFailedException;
  factory _QueryFailedException.fromJson(Map<String, dynamic> json) =>
      _$_QueryFailedExceptionFromJson(json);
  const _QueryFailedException._();
}

@freezed
abstract class RecordedQueryCall implements _$RecordedQueryCall {
  const factory RecordedQueryCall({
    @required String name,
    @required Object key,
    @required List<RecordedQueryCall> innerCalls,
    Object result,
    List<ReportedCompilerError> thrownErrors,
  }) = _RecordedQueryCall;
  factory RecordedQueryCall.fromJson(Map<String, dynamic> json) =>
      _$RecordedQueryCallFromJson(json);
  const RecordedQueryCall._();
}

