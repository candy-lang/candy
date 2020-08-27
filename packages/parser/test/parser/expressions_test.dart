import 'package:meta/meta.dart';
import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/expressions/literal.dart';
import 'package:parser/src/parser/ast/node.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:petitparser/petitparser.dart';
import 'package:test/test.dart';

void main() {
  setUpAll(ParserGrammar.init);

  group('literals', () {
    group('IntegerLiteral', () {
      // TODO(JonasWanke): negative literals
      tableTestExpressionParser<Literal<int>>(
        'decimal',
        table: {
          '0': 0,
          '1': 1,
          '01': 1,
          '2': 2,
          '10': 10,
          '1_0': 10,
          '100': 100,
          '10_0': 100,
          '1_0_0': 100,
          '1000': 1000,
          '1_000': 1000,
        }.map((key, value) {
          final newValue = Literal(
            IntegerLiteralToken(value, span: SourceSpan(0, key.length)),
          );
          return MapEntry(key, newValue);
        }),
      );
      tableTestExpressionParser<Literal<int>>(
        'hexadecimal',
        table: {
          '0x0': 0,
          '0x1': 1,
          '0x2': 2,
          '0x10': 0x10,
          '0x1_0': 0x10,
          '0x100': 0x100,
          '0x10_0': 0x100,
          '0x1_0_0': 0x100,
          '0x1000': 0x1000,
          '0x1_000': 0x1000,
        }.map((key, value) {
          final newValue = Literal(
            IntegerLiteralToken(value, span: SourceSpan(0, key.length)),
          );
          return MapEntry(key, newValue);
        }),
      );
      tableTestExpressionParser<Literal<int>>(
        'binary',
        table: {
          '0b0': 0x0,
          '0b1': 0x1,
          '0b10': 0x2,
          '0b1_0': 0x2,
          '0b100': 0x4,
          '0b10_0': 0x4,
          '0b1_0_0': 0x4,
          '0b1000': 0x8,
          '0b1_000': 0x8,
        }.map((key, value) {
          final newValue = Literal(
            IntegerLiteralToken(value, span: SourceSpan(0, key.length)),
          );
          return MapEntry(key, newValue);
        }),
      );
    });
    tableTestExpressionParser<Literal<bool>>(
      'BooleanLiteral',
      table: {
        'true': true,
        'false': false,
      }.map((key, value) {
        final newValue = Literal(
          BooleanLiteralToken(value, span: SourceSpan(0, key.length)),
        );
        return MapEntry(key, newValue);
      }),
    );
  });
}

@isTestGroup
void tableTest<T, R>(
  String description, {
  @required Map<T, R> table,
  @required R Function(T value) converter,
}) {
  assert(table != null);
  assert(converter != null);

  group(description, () {
    for (final entry in table.entries) {
      test(entry.key, () => expect(converter(entry.key), equals(entry.value)));
    }
  });
}

@isTestGroup
void tableTestParser<R extends AstNode>(
  String description, {
  @required Map<String, R> table,
  @required Parser parser,
}) {
  assert(table != null);
  assert(parser != null);

  tableTest<String, R>(
    description,
    table: table,
    converter: (source) {
      final result = parser.parse(source);
      expect(result.isSuccess, isTrue);
      expect(
        result.position,
        source.length,
        reason: "Didn't match the whole input string.",
      );
      return result.value as R;
    },
  );
}

@isTestGroup
void tableTestExpressionParser<R extends AstNode>(
  String description, {
  @required Map<String, R> table,
}) {
  assert(table != null);

  tableTestParser<R>(
    description,
    table: table,
    parser: ParserGrammar.expression,
  );
}
