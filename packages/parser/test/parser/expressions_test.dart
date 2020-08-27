import 'package:meta/meta.dart';
import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/expressions/expression.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:parser/src/syntactic_entity.dart';
import 'package:test/test.dart';

import 'utils.dart';

void main() {
  setUpAll(ParserGrammar.init);

  group('primitive', () {
    group('literals', () {
      group('IntegerLiteral', () {
        tableTestExpressionParser<int, Literal<int>>(
          'decimal',
          table: validDecIntegerLiterals,
          nodeMapper: (value, fullSpan) =>
              Literal(IntegerLiteralToken(value, span: fullSpan)),
        );
        tableTestExpressionParser<int, Literal<int>>(
          'hexadecimal',
          table: validHexIntegerLiterals,
          nodeMapper: (value, fullSpan) =>
              Literal(IntegerLiteralToken(value, span: fullSpan)),
        );
        tableTestExpressionParser<int, Literal<int>>(
          'binary',
          table: validBinIntegerLiterals,
          nodeMapper: (value, fullSpan) =>
              Literal(IntegerLiteralToken(value, span: fullSpan)),
        );
      });
      tableTestExpressionParser<bool, Literal<bool>>(
        'BooleanLiteral',
        table: validBooleanLiterals,
        nodeMapper: (value, fullSpan) =>
            Literal(BooleanLiteralToken(value, span: fullSpan)),
      );
    });

    tableTestExpressionParser<String, Identifier>(
      'identifiers',
      table: Map.fromIterable(validIdentifiers),
      nodeMapper: (value, fullSpan) =>
          Identifier(IdentifierToken(value, span: fullSpan)),
    );
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

    group('navigation', () {
      forAllPrimitives(tester: (primitiveSource, primitiveFactory) {
        forAll<String>(
          table: validIdentifiers,
          tester: (identifier) {
            testExpressionParser(
              '$primitiveSource.$identifier',
              expression: NavigationExpression(
                target: primitiveFactory(0),
                dot: OperatorToken(
                  OperatorTokenType.dot,
                  span: SourceSpan(
                    primitiveSource.length,
                    primitiveSource.length + 1,
                  ),
                ),
                name: IdentifierToken(
                  identifier,
                  span: SourceSpan(
                    primitiveSource.length + 1,
                    primitiveSource.length + 1 + identifier.length,
                  ),
                ),
              ),
            );
          },
        );
      });
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

// TODO(JonasWanke): negative literals
final validDecIntegerLiterals = {
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
};
final validHexIntegerLiterals = {
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
};
final validBinIntegerLiterals = {
  '0b0': 0x0,
  '0b1': 0x1,
  '0b10': 0x2,
  '0b1_0': 0x2,
  '0b100': 0x4,
  '0b10_0': 0x4,
  '0b1_0_0': 0x4,
  '0b1000': 0x8,
  '0b1_000': 0x8,
};
final validIntegerLiterals = {
  ...validDecIntegerLiterals,
  ...validHexIntegerLiterals,
  ...validHexIntegerLiterals,
};
final validBooleanLiterals = {
  'true': true,
  'false': false,
};
final validIdentifiers = [
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
];

typedef PrimitiveFactory = Expression Function(int startOffset);
typedef PrimitiveTester = void Function(
  String source,
  PrimitiveFactory primitiveFactory,
);
@isTestGroup
void forAllPrimitives({@required PrimitiveTester tester}) {
  assert(tester != null);

  final integerLiterals =
      validIntegerLiterals.map<String, PrimitiveFactory>((source, value) {
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
  final booleanLiterals =
      validBooleanLiterals.map<String, PrimitiveFactory>((source, value) {
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
    validIdentifiers,
    value: (dynamic source) => (offset) {
      return Identifier(
        IdentifierToken(
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
