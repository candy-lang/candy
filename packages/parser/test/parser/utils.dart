import 'package:meta/meta.dart';
import 'package:parser/src/source_span.dart';
import 'package:parser/src/syntactic_entity.dart';
import 'package:petitparser/petitparser.dart';
import 'package:test/test.dart';

@isTestGroup
void forAll<T>({
  @required Iterable<T> table,
  @required void Function(T value) tester,
}) {
  assert(table != null);
  assert(tester != null);

  table.forEach(tester);
}

@isTestGroup
void forAllMap<K, V>({
  @required Map<K, V> table,
  @required void Function(K key, V value) tester,
}) {
  assert(table != null);
  assert(tester != null);

  table.forEach(tester);
}

@isTestGroup
void tableTestParser<R, N extends SyntacticEntity>(
  String description, {
  @required Map<String, R> table,
  @required N Function(R raw, SourceSpan fullSpan) nodeMapper,
  @required Parser parser,
}) {
  assert(table != null);
  assert(parser != null);

  group(description, () {
    forAll<MapEntry<String, R>>(
      table: table.entries,
      tester: (entry) {
        final source = entry.key;
        test(source, () {
          final node = nodeMapper(entry.value, SourceSpan(0, source.length));

          final result = parser.parse(source);
          expect(result.isSuccess, isTrue);
          expect(
            result.position,
            source.length,
            reason: "Didn't match the whole input string.",
          );
          expect(result.value, equals(node));
        });
      },
    );
  });
}
