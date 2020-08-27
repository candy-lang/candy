import 'package:meta/meta.dart';
import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/expressions/expression.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:parser/src/syntactic_entity.dart';
import 'package:petitparser/petitparser.dart';
import 'package:test/test.dart';

void main() {
  setUpAll(ParserGrammar.init);

  tableTestExpressionParser<String, Identifier>(
    'identifiers',
    table: Map.fromIterable(<String>[
      'a',
      'a123',
      'aa',
      'aa123',
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa123',
      'A',
      'A123',
      'AA',
      'AA123',
      'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
      'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA123',
      '_',
      '_123',
      '__',
      '__123',
      '_______________________________',
      '_______________________________123',
    ]),
    nodeMapper: (value, fullSpan) =>
        Identifier(SimpleIdentifierToken(value, span: fullSpan)),
  );

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
        nodeMapper: (value, fullSpan) =>
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
        nodeMapper: (value, fullSpan) =>
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
        nodeMapper: (value, fullSpan) =>
            Literal(IntegerLiteralToken(value, span: fullSpan)),
      );
    });
    tableTestExpressionParser<bool, Literal<bool>>(
      'BooleanLiteral',
      table: {
        'true': true,
        'false': false,
      },
      nodeMapper: (value, fullSpan) =>
          Literal(BooleanLiteralToken(value, span: fullSpan)),
    );
  });

  group('grouping', () {
    group('literals', () {
      tableTestExpressionParser<LiteralToken<int>, ParenthesizedExpression>(
        'integer',
        table: {
          '(0)': IntegerLiteralToken(0, span: SourceSpan(1, 2)),
          '(1)': IntegerLiteralToken(1, span: SourceSpan(1, 2)),
          '(01)': IntegerLiteralToken(1, span: SourceSpan(1, 3)),
          '(2)': IntegerLiteralToken(2, span: SourceSpan(1, 2)),
          '(10)': IntegerLiteralToken(10, span: SourceSpan(1, 3)),
          '(1_0)': IntegerLiteralToken(10, span: SourceSpan(1, 4)),
          '(100)': IntegerLiteralToken(100, span: SourceSpan(1, 4)),
        },
        nodeMapper: (value, fullSpan) => ParenthesizedExpression(
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
          expression: Literal<int>(value),
          rightParenthesis: OperatorToken(
            OperatorTokenType.rparen,
            span: SourceSpan(value.span.end, value.span.end + 1),
          ),
        ),
      );
    });
    tableTestExpressionParser<String, ParenthesizedExpression>(
      'identifiers',
      table: Map.fromIterable(
        <String>['a', 'a123', 'A', 'A123', '_', '_123'],
        key: (dynamic v) => '($v)',
      ),
      nodeMapper: (value, fullSpan) => ParenthesizedExpression(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        expression: Identifier(
          SimpleIdentifierToken(value, span: SourceSpan(1, value.length + 1)),
        ),
        rightParenthesis: OperatorToken(
          OperatorTokenType.rparen,
          span: SourceSpan(value.length + 1, value.length + 2),
        ),
      ),
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
void tableTestParser<R, N extends SyntacticEntity>(
  String description, {
  @required Map<String, R> table,
  @required N Function(R raw, SourceSpan fullSpan) nodeMapper,
  @required Parser parser,
}) {
  assert(table != null);
  assert(parser != null);

  tableTest<String, N>(
    description,
    table: table.map((key, value) {
      return MapEntry(key, nodeMapper(value, SourceSpan(0, key.length)));
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
void tableTestExpressionParser<R, N extends SyntacticEntity>(
  String description, {
  @required Map<String, R> table,
  @required N Function(R raw, SourceSpan fullSpan) nodeMapper,
}) {
  tableTestParser<R, N>(
    description,
    table: table,
    nodeMapper: nodeMapper,
    parser: ParserGrammar.expression,
  );
}
