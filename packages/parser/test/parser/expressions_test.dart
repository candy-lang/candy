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
      tableTestExpressionParser<int, Literal<int>>(
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
        },
        astMapper: (value, fullSpan) =>
            Literal(IntegerLiteralToken(value, span: fullSpan)),
      );
      tableTestExpressionParser<int, Literal<int>>(
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
        },
        astMapper: (value, fullSpan) =>
            Literal(IntegerLiteralToken(value, span: fullSpan)),
      );
      tableTestExpressionParser<int, Literal<int>>(
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
        },
        astMapper: (value, fullSpan) =>
            Literal(IntegerLiteralToken(value, span: fullSpan)),
      );
    });
    tableTestExpressionParser<bool, Literal<bool>>(
      'BooleanLiteral',
      table: {
        'true': true,
        'false': false,
      },
      astMapper: (value, fullSpan) =>
          Literal(BooleanLiteralToken(value, span: fullSpan)),
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
void tableTestParser<R, N extends AstNode>(
  String description, {
  @required Map<String, R> table,
  @required N Function(R raw, SourceSpan fullSpan) astMapper,
  @required Parser parser,
}) {
  assert(table != null);
  assert(parser != null);

  tableTest<String, N>(
    description,
    table: table.map((key, value) {
      return MapEntry(key, astMapper(value, SourceSpan(0, key.length)));
    }),
    converter: (source) {
      final result = parser.parse(source);
      expect(result.isSuccess, isTrue);
      expect(
        result.position,
        source.length,
        reason: "Didn't match the whole input string.",
      );
      return result.value as N;
    },
  );
}

@isTestGroup
void tableTestExpressionParser<R, N extends AstNode>(
  String description, {
  @required Map<String, R> table,
  @required N Function(R raw, SourceSpan fullSpan) astMapper,
}) {
  tableTestParser<R, N>(
    description,
    table: table,
    astMapper: astMapper,
    parser: ParserGrammar.expression,
  );
}
