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

  group('primitive', () {
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
  });

  group('grouping', () {
    forAllPrimitives(
      tester: (source, primitiveFactory) {
        final primitive = primitiveFactory(1);
        testExpressionParser(
          '($source)',
          expression: ParenthesizedExpression(
            leftParenthesis:
                OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
            expression: primitive,
            rightParenthesis: OperatorToken(
              OperatorTokenType.rparen,
              span: SourceSpan(source.length + 1, source.length + 2),
            ),
          ),
        );
      },
    );
  });

  group('unary postfix', () {
    group('simple operators', () {
      forAllMap<String, OperatorTokenType>(
        table: <String, OperatorTokenType>{
          '++': OperatorTokenType.plusPlus,
          '--': OperatorTokenType.minusMinus,
          '?': OperatorTokenType.question,
          '!': OperatorTokenType.exclamation,
        },
        tester: (operatorSource, operatorType) {
          group(operatorType.toString(), () {
            forAllPrimitives(tester: (primitiveSource, primitiveFactory) {
              testExpressionParser(
                '$primitiveSource$operatorSource',
                expression: PostfixExpression(
                  operand: primitiveFactory(0),
                  operatorToken: OperatorToken(
                    operatorType,
                    span: SourceSpan(
                      primitiveSource.length,
                      primitiveSource.length + operatorSource.length,
                    ),
                  ),
                ),
              );
            });
          });
        },
      );
    });
  });

  group('unary prefix', () {
    group('simple operators', () {
      forAllMap<String, OperatorTokenType>(
        table: <String, OperatorTokenType>{
          '-': OperatorTokenType.minus,
          '!': OperatorTokenType.exclamation,
          '~': OperatorTokenType.tilde,
          '++': OperatorTokenType.plusPlus,
          '--': OperatorTokenType.minusMinus,
        },
        tester: (operatorSource, operatorType) {
          group(operatorType.toString(), () {
            forAllPrimitives(tester: (primitiveSource, primitiveFactory) {
              testExpressionParser(
                '$operatorSource$primitiveSource',
                expression: PrefixExpression(
                  operatorToken: OperatorToken(
                    operatorType,
                    span: SourceSpan(0, operatorSource.length),
                  ),
                  operand: primitiveFactory(operatorSource.length),
                ),
              );
            });
          });
        },
      );
    });
  });
}

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

typedef PrimitiveFactory = Expression Function(int startOffset);
typedef PrimitiveTester = void Function(
  String source,
  PrimitiveFactory primitiveFactory,
);
@isTestGroup
void forAllPrimitives({@required PrimitiveTester tester}) {
  assert(tester != null);

  final integerLiterals = {
    // decimal
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
    // hexadecimal
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
    // binary
    '0b0': 0x0,
    '0b1': 0x1,
    '0b10': 0x2,
    '0b1_0': 0x2,
    '0b100': 0x4,
    '0b10_0': 0x4,
    '0b1_0_0': 0x4,
    '0b1000': 0x8,
    '0b1_000': 0x8,
  }.map<String, PrimitiveFactory>((source, value) {
    return MapEntry(
      source,
      (offset) => Literal<int>(
        IntegerLiteralToken(
          value,
          span: SourceSpan(0, source.length).plus(offset),
        ),
      ),
    );
  });
  final booleanLiterals = {
    'true': true,
    'false': false,
  }.map<String, PrimitiveFactory>((source, value) {
    return MapEntry(
      source,
      (offset) => Literal<bool>(
        BooleanLiteralToken(
          value,
          span: SourceSpan(0, source.length).plus(offset),
        ),
      ),
    );
  });
  final identifierLiterals = Map<String, PrimitiveFactory>.fromIterable(
    <String>[
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
    ],
    value: (dynamic source) => (offset) {
      return Identifier(
        SimpleIdentifierToken(
          source as String,
          span: SourceSpan(0, (source as String).length).plus(offset),
        ),
      );
    },
  );

  forAll<MapEntry<String, PrimitiveFactory>>(
    table: <String, PrimitiveFactory>{
      ...integerLiterals,
      ...booleanLiterals,
      ...identifierLiterals,
    }.entries,
    tester: (entry) => tester(entry.key, entry.value),
  );
}

@isTest
void testExpressionParser(
  String source, {
  @required Expression expression,
}) {
  assert(expression != null);

  test(source, () {
    final result = ParserGrammar.expression.parse(source);
    expect(result.isSuccess, isTrue);
    expect(
      result.position,
      source.length,
      reason: "Didn't match the whole input string.",
    );
    expect(result.value, equals(expression));
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
