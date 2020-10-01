import 'package:meta/meta.dart';
import 'package:parser/parser.dart';
import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/expressions/expressions.dart';
import 'package:parser/src/parser/ast/statements.dart';
import 'package:parser/src/parser/ast/types.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:parser/src/syntactic_entity.dart';
import 'package:petitparser/parser.dart';
import 'package:test/test.dart';

import 'statements_test.dart';
import 'types_test.dart';
import 'utils.dart';

void main() {
  setUp(ParserGrammar.init);

  group('primitive', () {
    group('literals', () {
      group('IntegerLiteral', () {
        tableTestExpressionParser<int, Literal<int>>(
          'decimal',
          table: validDecIntegerLiterals,
          nodeMapper: (value, fullSpan) =>
              Literal(0, IntegerLiteralToken(value, span: fullSpan)),
        );
        tableTestExpressionParser<int, Literal<int>>(
          'hexadecimal',
          table: validHexIntegerLiterals,
          nodeMapper: (value, fullSpan) =>
              Literal(0, IntegerLiteralToken(value, span: fullSpan)),
        );
        tableTestExpressionParser<int, Literal<int>>(
          'binary',
          table: validBinIntegerLiterals,
          nodeMapper: (value, fullSpan) =>
              Literal(0, IntegerLiteralToken(value, span: fullSpan)),
        );
      });
      tableTestExpressionParser<bool, Literal<bool>>(
        'BooleanLiteral',
        table: validBooleanLiterals,
        nodeMapper: (value, fullSpan) =>
            Literal(0, BooleanLiteralToken(value, span: fullSpan)),
      );
    });

    tableTestExpressionParser<LambdaLiteral, LambdaLiteral>(
      'lambda literals',
      table: {
        '{}': LambdaLiteral(
          0,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(0, 1)),
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(1, 2)),
        ),
        '{ it }': LambdaLiteral(
          1,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(0, 1)),
          statements: [
            Identifier(0, IdentifierToken('it', span: SourceSpan(2, 4))),
          ],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(5, 6)),
        ),
        '{ foo => foo }': LambdaLiteral(
          1,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(0, 1)),
          valueParameters: [
            ValueParameter(
              name: IdentifierToken('foo', span: SourceSpan(2, 5)),
            ),
          ],
          arrow: OperatorToken(
            OperatorTokenType.equalsGreater,
            span: SourceSpan(6, 8),
          ),
          statements: [
            Identifier(0, IdentifierToken('foo', span: SourceSpan(9, 12))),
          ],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(13, 14)),
        ),
        '{ foo: Foo, bar => foo }': LambdaLiteral(
          1,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(0, 1)),
          valueParameters: [
            ValueParameter(
              name: IdentifierToken('foo', span: SourceSpan(2, 5)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(5, 6),
              ),
              type: UserType(simpleTypes: [
                SimpleUserType(IdentifierToken('Foo', span: SourceSpan(7, 10))),
              ]),
            ),
            ValueParameter(
              name: IdentifierToken('bar', span: SourceSpan(12, 15)),
            ),
          ],
          valueParameterCommata: [
            OperatorToken(OperatorTokenType.comma, span: SourceSpan(10, 11)),
          ],
          arrow: OperatorToken(
            OperatorTokenType.equalsGreater,
            span: SourceSpan(16, 18),
          ),
          statements: [
            Identifier(0, IdentifierToken('foo', span: SourceSpan(19, 22))),
          ],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(23, 24)),
        ),
      },
      nodeMapper: (value, _) => value,
    );

    tableTestExpressionParser<String, Identifier>(
      'identifiers',
      table: Map.fromIterable(validIdentifiers),
      nodeMapper: (value, fullSpan) =>
          Identifier(0, IdentifierToken(value, span: fullSpan)),
    );
  });

  group('grouping', () {
    forPrimitives(
      0,
      tester: (source, primitiveFactory) {
        final primitive = primitiveFactory(1);
        testExpressionParser(
          '($source)',
          expression: GroupExpression(
            1,
            leftParenthesis:
                OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
            expression: primitive,
            rightParenthesis: OperatorToken(
              OperatorTokenType.rparen,
              span: SourceSpan.fromStartLength(source.length + 1, 1),
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
            forPrimitives(0, tester: (primitiveSource, primitiveFactory) {
              testExpressionParser(
                '$primitiveSource$operatorSource',
                expression: PostfixExpression(
                  1,
                  operand: primitiveFactory(0),
                  operatorToken: OperatorToken(
                    operatorType,
                    span: SourceSpan.fromStartLength(
                      primitiveSource.length,
                      operatorSource.length,
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
      forPrimitives(0, tester: (primitiveSource, primitiveFactory) {
        forAll<String>(
          table: validIdentifiers,
          tester: (identifier) {
            testExpressionParser(
              '$primitiveSource.$identifier',
              expression: NavigationExpression(
                1,
                target: primitiveFactory(0),
                dot: OperatorToken(
                  OperatorTokenType.dot,
                  span: SourceSpan.fromStartLength(primitiveSource.length, 1),
                ),
                name: IdentifierToken(
                  identifier,
                  span: SourceSpan.fromStartLength(
                    primitiveSource.length + 1,
                    identifier.length,
                  ),
                ),
              ),
            );
          },
        );
      });
    });

    group('call', () {
      group('positional', () {
        group('0 args', () {
          forPrimitives(0, tester: (targetSource, targetFactory) {
            testExpressionParser(
              '$targetSource()',
              expression: CallExpression(
                1,
                target: targetFactory(0),
                leftParenthesis: OperatorToken(
                  OperatorTokenType.lparen,
                  span: SourceSpan.fromStartLength(targetSource.length, 1),
                ),
                rightParenthesis: OperatorToken(
                  OperatorTokenType.rparen,
                  span: SourceSpan.fromStartLength(
                    targetSource.length + 1,
                    1,
                  ),
                ),
              ),
            );
          });
        });

        group('1 arg', () {
          forPrimitives(0, tester: (targetSource, targetFactory) {
            forPrimitives(1, tester: (arg1Source, arg1Factory) {
              testExpressionParser(
                '$targetSource($arg1Source)',
                expression: CallExpression(
                  2,
                  target: targetFactory(0),
                  leftParenthesis: OperatorToken(
                    OperatorTokenType.lparen,
                    span: SourceSpan.fromStartLength(targetSource.length, 1),
                  ),
                  arguments: [
                    Argument(expression: arg1Factory(targetSource.length + 1)),
                  ],
                  rightParenthesis: OperatorToken(
                    OperatorTokenType.rparen,
                    span: SourceSpan.fromStartLength(
                      targetSource.length + 1 + arg1Source.length,
                      1,
                    ),
                  ),
                ),
              );
            });
          });
        });

        group('2 args', () {
          forPrimitives(0, tester: (targetSource, targetFactory) {
            forPrimitives(1, tester: (arg1Source, arg1Factory) {
              forPrimitives(2, tester: (arg2Source, arg2Factory) {
                testExpressionParser(
                  '$targetSource($arg1Source, $arg2Source)',
                  expression: CallExpression(
                    3,
                    target: targetFactory(0),
                    leftParenthesis: OperatorToken(
                      OperatorTokenType.lparen,
                      span: SourceSpan.fromStartLength(targetSource.length, 1),
                    ),
                    arguments: [
                      Argument(
                        expression: arg1Factory(targetSource.length + 1),
                      ),
                      Argument(
                        expression: arg2Factory(
                          targetSource.length + 1 + arg1Source.length + 2,
                        ),
                      ),
                    ],
                    argumentCommata: [
                      OperatorToken(
                        OperatorTokenType.comma,
                        span: SourceSpan.fromStartLength(
                          targetSource.length + 1 + arg1Source.length,
                          1,
                        ),
                      ),
                    ],
                    rightParenthesis: OperatorToken(
                      OperatorTokenType.rparen,
                      span: SourceSpan.fromStartLength(
                        targetSource.length +
                            1 +
                            arg1Source.length +
                            2 +
                            arg2Source.length,
                        1,
                      ),
                    ),
                  ),
                );
              });
            });
          });
        });
      });
      group('with type arguments', () {
        forPrimitives(0, tester: (targetSource, targetFactory) {
          testExpressionParser(
            '$targetSource<Foo.Bar, Foo.Bar>()',
            expression: CallExpression(
              1,
              target: targetFactory(0),
              typeArguments: TypeArguments(
                leftAngle: OperatorToken(
                  OperatorTokenType.langle,
                  span: SourceSpan.fromStartLength(targetSource.length, 1),
                ),
                arguments: [
                  TypeArgument(type: createTypeFooBar(targetSource.length + 1)),
                  TypeArgument(
                    type: createTypeFooBar(targetSource.length + 10),
                  ),
                ],
                commata: [
                  OperatorToken(
                    OperatorTokenType.comma,
                    span:
                        SourceSpan.fromStartLength(targetSource.length + 8, 1),
                  ),
                ],
                rightAngle: OperatorToken(
                  OperatorTokenType.rangle,
                  span: SourceSpan.fromStartLength(targetSource.length + 17, 1),
                ),
              ),
              leftParenthesis: OperatorToken(
                OperatorTokenType.lparen,
                span: SourceSpan.fromStartLength(targetSource.length + 18, 1),
              ),
              rightParenthesis: OperatorToken(
                OperatorTokenType.rparen,
                span: SourceSpan.fromStartLength(targetSource.length + 19, 1),
              ),
            ),
          );
        });
      });
    });
  });

  group('unary prefix', () {
    group('simple operators', () {
      forAllMap<String, OperatorTokenType>(
        table: {
          '-': OperatorTokenType.minus,
          '!': OperatorTokenType.exclamation,
          '~': OperatorTokenType.tilde,
          '++': OperatorTokenType.plusPlus,
          '--': OperatorTokenType.minusMinus,
        },
        tester: (operatorSource, operatorType) {
          group(operatorType.toString(), () {
            forPrimitives(0, tester: (primitiveSource, primitiveFactory) {
              testExpressionParser(
                '$operatorSource$primitiveSource',
                expression: PrefixExpression(
                  1,
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

  tableTestExpressionParser<IfExpression, IfExpression>(
    'if',
    table: {
      'if (true) 123': IfExpression(
        3,
        ifKeyword: KeywordToken.if_(span: SourceSpan(0, 2)) as IfKeywordToken,
        condition: GroupExpression(
          1,
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(3, 4)),
          expression: Literal<bool>(
            0,
            BooleanLiteralToken(true, span: SourceSpan(4, 8)),
          ),
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(8, 9)),
        ),
        thenStatement: createStatement123(2, 10),
      ),
      'if true { 123 }': IfExpression(
        3,
        ifKeyword: KeywordToken.if_(span: SourceSpan(0, 2)) as IfKeywordToken,
        condition:
            Literal<bool>(0, BooleanLiteralToken(true, span: SourceSpan(3, 7))),
        thenStatement: Block(
          3,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(8, 9)),
          statements: [createStatement123(2, 10)],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(14, 15)),
        ),
      ),
      'if (true) 123 else 123': IfExpression(
        4,
        ifKeyword: KeywordToken.if_(span: SourceSpan(0, 2)) as IfKeywordToken,
        condition: GroupExpression(
          1,
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(3, 4)),
          expression: Literal<bool>(
            0,
            BooleanLiteralToken(true, span: SourceSpan(4, 8)),
          ),
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(8, 9)),
        ),
        thenStatement: createStatement123(2, 10),
        elseKeyword:
            KeywordToken.else_(span: SourceSpan(14, 18)) as ElseKeywordToken,
        elseStatement: createStatement123(3, 19),
      ),
      'if true { 123 } else { 123 }': IfExpression(
        5,
        ifKeyword: KeywordToken.if_(span: SourceSpan(0, 2)) as IfKeywordToken,
        condition:
            Literal<bool>(0, BooleanLiteralToken(true, span: SourceSpan(3, 7))),
        thenStatement: Block(
          0,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(8, 9)),
          statements: [createStatement123(10)],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(14, 15)),
        ),
        elseKeyword:
            KeywordToken.else_(span: SourceSpan(16, 20)) as ElseKeywordToken,
        elseStatement: Block(
          0,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(21, 22)),
          statements: [createStatement123(23)],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(27, 28)),
        ),
      ),
      'if (true) 123 else if false { 123 } else 123': IfExpression(
        0,
        ifKeyword: KeywordToken.if_(span: SourceSpan(0, 2)) as IfKeywordToken,
        condition: GroupExpression(
          0,
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(3, 4)),
          expression: Literal<bool>(
              0, BooleanLiteralToken(true, span: SourceSpan(4, 8))),
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(8, 9)),
        ),
        thenStatement: createStatement123(10),
        elseKeyword:
            KeywordToken.else_(span: SourceSpan(14, 18)) as ElseKeywordToken,
        elseStatement: IfExpression(
          0,
          ifKeyword:
              KeywordToken.if_(span: SourceSpan(19, 21)) as IfKeywordToken,
          condition: Literal<bool>(
            0,
            BooleanLiteralToken(false, span: SourceSpan(22, 27)),
          ),
          thenStatement: Block(
            0,
            leftBrace: OperatorToken(
              OperatorTokenType.lcurl,
              span: SourceSpan(28, 29),
            ),
            statements: [createStatement123(30)],
            rightBrace: OperatorToken(
              OperatorTokenType.rcurl,
              span: SourceSpan(34, 35),
            ),
          ),
          elseKeyword:
              KeywordToken.else_(span: SourceSpan(36, 40)) as ElseKeywordToken,
          elseStatement: createStatement123(41),
        ),
      ),
    },
    nodeMapper: (ifExpression, _) => ifExpression,
  );
}

final someValidDecIntegerLiterals = {
  '0': 0,
  '1': 1,
  '1000': 1000,
};
final validDecIntegerLiterals = {
  ...someValidDecIntegerLiterals,
  '01': 1,
  '2': 2,
  '10': 10,
  '1_0': 10,
  '100': 100,
  '10_0': 100,
  '1_0_0': 100,
  '1_000': 1000,
};
final someValidHexIntegerLiterals = {
  '0x0': 0,
  '0x10': 0x10,
};
final validHexIntegerLiterals = {
  ...someValidHexIntegerLiterals,
  '0x1': 1,
  '0x2': 2,
  '0x1_0': 0x10,
  '0x100': 0x100,
  '0x10_0': 0x100,
  '0x1_0_0': 0x100,
  '0x1000': 0x1000,
  '0x1_000': 0x1000,
};
final someValidBinIntegerLiterals = {
  '0b0': 0x0,
  '0b10': 0x2,
};
final validBinIntegerLiterals = {
  ...someValidBinIntegerLiterals,
  '0b1': 0x1,
  '0b1_0': 0x2,
  '0b100': 0x4,
  '0b10_0': 0x4,
  '0b1_0_0': 0x4,
  '0b1000': 0x8,
  '0b1_000': 0x8,
};
final someValidIntegerLiterals = {
  ...someValidDecIntegerLiterals,
  // Hexadecimal and binary are completely removed for performance reasons.
};
final validIntegerLiterals = {
  ...validDecIntegerLiterals,
  ...validHexIntegerLiterals,
  ...validBinIntegerLiterals,
};
final someValidBooleanLiterals = {
  'true': true,
};
final validBooleanLiterals = {
  ...someValidBooleanLiterals,
  'false': false,
};
final someValidIdentifiers = [
  'a',
  'A123',
  '_',
];
final validIdentifiers = [
  ...someValidIdentifiers,
  'a123',
  'aa',
  'aa123',
  'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
  'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa123',
  'A',
  'AA',
  'AA123',
  'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
  'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA123',
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
void forPrimitives(int id, {@required PrimitiveTester tester}) {
  assert(tester != null);

  final integerLiterals =
      someValidIntegerLiterals.map<String, PrimitiveFactory>((source, value) {
    return MapEntry(
      source,
      (offset) => Literal<int>(
        id,
        IntegerLiteralToken(
          value,
          span: SourceSpan(0, source.length).plus(offset),
        ),
      ),
    );
  });
  final booleanLiterals =
      someValidBooleanLiterals.map<String, PrimitiveFactory>((source, value) {
    return MapEntry(
      source,
      (offset) => Literal<bool>(
        id,
        BooleanLiteralToken(
          value,
          span: SourceSpan(0, source.length).plus(offset),
        ),
      ),
    );
  });
  final identifierLiterals = Map<String, PrimitiveFactory>.fromIterable(
    someValidIdentifiers,
    value: (dynamic source) => (offset) {
      return Identifier(
        id,
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
void testExpressionParser(String source, {@required Expression expression}) {
  assert(expression != null);

  testParser(source, result: expression, parser: ParserGrammar.expression);
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
    parser: (ParserGrammar.expression & endOfInput())
        .map<Expression>((value) => value[0] as Expression),
  );
}
