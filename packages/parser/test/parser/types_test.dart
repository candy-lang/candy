import 'package:meta/meta.dart';
import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/types.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:test/test.dart';

import 'utils.dart';

void main() {
  setUpAll(ParserGrammar.init);

  tableTestTypeParser<UserType>(
    'UserType',
    table: {
      'Foo': UserType(simpleTypes: [
        SimpleUserType(IdentifierToken('Foo', span: SourceSpan(0, 3))),
      ]),
      'Float64': UserType(simpleTypes: [
        SimpleUserType(IdentifierToken('Float64', span: SourceSpan(0, 7))),
      ]),
      'My.Nested.Type': UserType(
        simpleTypes: [
          SimpleUserType(IdentifierToken('My', span: SourceSpan(0, 2))),
          SimpleUserType(IdentifierToken('Nested', span: SourceSpan(3, 9))),
          SimpleUserType(IdentifierToken('Type', span: SourceSpan(10, 14))),
        ],
        dots: [
          OperatorToken(OperatorTokenType.dot, span: SourceSpan(2, 3)),
          OperatorToken(OperatorTokenType.dot, span: SourceSpan(9, 10)),
        ],
      ),
      'My.Type<With, Arguments>': UserType(
        simpleTypes: [
          SimpleUserType(IdentifierToken('My', span: SourceSpan(0, 2))),
          SimpleUserType(IdentifierToken('Type', span: SourceSpan(3, 7))),
        ],
        dots: [OperatorToken(OperatorTokenType.dot, span: SourceSpan(2, 3))],
        arguments: TypeArguments(
          leftAngle:
              OperatorToken(OperatorTokenType.langle, span: SourceSpan(7, 8)),
          arguments: [
            TypeArgument(
              type: UserType(
                simpleTypes: [
                  SimpleUserType(
                    IdentifierToken('With', span: SourceSpan(8, 12)),
                  ),
                ],
              ),
            ),
            TypeArgument(
              type: UserType(
                simpleTypes: [
                  SimpleUserType(
                    IdentifierToken('Arguments', span: SourceSpan(14, 23)),
                  ),
                ],
              ),
            ),
          ],
          commata: [
            OperatorToken(OperatorTokenType.comma, span: SourceSpan(12, 13)),
          ],
          rightAngle:
              OperatorToken(OperatorTokenType.rangle, span: SourceSpan(23, 24)),
        ),
      ),
    },
  );

  tableTestTypeParser<GroupType>(
    'GroupType',
    table: {
      '(Foo.Bar)': GroupType(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        type: createTypeFooBar(1),
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(8, 9)),
      ),
      '((Foo.Bar))': GroupType(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        type: GroupType(
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(1, 2)),
          type: createTypeFooBar(2),
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(9, 10)),
        ),
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(10, 11)),
      ),
    },
  );

  tableTestTypeParser<TupleType>(
    'TupleType',
    table: {
      '(Foo.Bar, Foo.Bar)': TupleType(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        types: [createTypeFooBar(1), createTypeFooBar(10)],
        commata: [
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(8, 9)),
        ],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(17, 18)),
      ),
      '(Foo.Bar ,Foo.Bar\n, Foo.Bar ,)': TupleType(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        types: [
          createTypeFooBar(1),
          createTypeFooBar(10),
          createTypeFooBar(20),
        ],
        commata: [
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(9, 10)),
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(18, 19)),
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(28, 29)),
        ],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(29, 30)),
      ),
      '(Foo.Bar, Foo.Bar, Foo.Bar, Foo.Bar, Foo.Bar)': TupleType(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        types: [
          createTypeFooBar(1),
          createTypeFooBar(10),
          createTypeFooBar(19),
          createTypeFooBar(28),
          createTypeFooBar(37),
        ],
        commata: [
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(8, 9)),
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(17, 18)),
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(26, 27)),
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(35, 36)),
        ],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(44, 45)),
      ),
    },
  );

  tableTestTypeParser<Type>(
    'Union & Intersection types',
    table: {
      'Foo.Bar & Foo.Bar': IntersectionType(
        leftType: createTypeFooBar(),
        ampersand: OperatorToken(
          OperatorTokenType.ampersand,
          span: SourceSpan(8, 9),
        ),
        rightType: createTypeFooBar(10),
      ),
      'Foo.Bar | Foo.Bar': UnionType(
        leftType: createTypeFooBar(),
        bar: OperatorToken(OperatorTokenType.bar, span: SourceSpan(8, 9)),
        rightType: createTypeFooBar(10),
      ),
      'Foo.Bar | Foo.Bar | Foo.Bar': UnionType(
        leftType: UnionType(
          leftType: createTypeFooBar(),
          bar: OperatorToken(OperatorTokenType.bar, span: SourceSpan(8, 9)),
          rightType: createTypeFooBar(10),
        ),
        bar: OperatorToken(OperatorTokenType.bar, span: SourceSpan(18, 19)),
        rightType: createTypeFooBar(20),
      ),
      'Foo.Bar & Foo.Bar | Foo.Bar & Foo.Bar': UnionType(
        leftType: IntersectionType(
          leftType: createTypeFooBar(),
          ampersand: OperatorToken(
            OperatorTokenType.ampersand,
            span: SourceSpan(8, 9),
          ),
          rightType: createTypeFooBar(10),
        ),
        bar: OperatorToken(OperatorTokenType.bar, span: SourceSpan(18, 19)),
        rightType: IntersectionType(
          leftType: createTypeFooBar(20),
          ampersand: OperatorToken(
            OperatorTokenType.ampersand,
            span: SourceSpan(28, 29),
          ),
          rightType: createTypeFooBar(30),
        ),
      ),
    },
  );

  tableTestTypeParser<FunctionType>(
    'FunctionType',
    table: {
      '() => Foo.Bar': FunctionType(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        parameterTypes: [],
        parameterCommata: [],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(1, 2)),
        arrow: OperatorToken(
          OperatorTokenType.equalsGreater,
          span: SourceSpan(3, 5),
        ),
        returnType: createTypeFooBar(6),
      ),
      '(Foo.Bar) => Foo.Bar': FunctionType(
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
        parameterTypes: [createTypeFooBar(1)],
        parameterCommata: [],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(8, 9)),
        arrow: OperatorToken(
          OperatorTokenType.equalsGreater,
          span: SourceSpan(10, 12),
        ),
        returnType: createTypeFooBar(13),
      ),
      'Foo.Bar.() => Foo.Bar': FunctionType(
        receiver: createTypeFooBar(),
        receiverDot:
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(7, 8)),
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(8, 9)),
        parameterTypes: [],
        parameterCommata: [],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(9, 10)),
        arrow: OperatorToken(
          OperatorTokenType.equalsGreater,
          span: SourceSpan(11, 13),
        ),
        returnType: createTypeFooBar(14),
      ),
    },
  );

  tableTestTypeParser(
    'complex',
    table: {
      '(Foo.Bar & Foo.Bar).(Foo.Bar, (Foo.Bar, Foo.Bar)) => Foo.Bar | Foo.Bar':
          FunctionType(
        receiver: GroupType(
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(0, 1)),
          type: IntersectionType(
            leftType: createTypeFooBar(1),
            ampersand: OperatorToken(
              OperatorTokenType.ampersand,
              span: SourceSpan(9, 10),
            ),
            rightType: createTypeFooBar(11),
          ),
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(18, 19)),
        ),
        receiverDot:
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(19, 20)),
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(20, 21)),
        parameterTypes: [
          createTypeFooBar(21),
          TupleType(
            leftParenthesis: OperatorToken(
              OperatorTokenType.lparen,
              span: SourceSpan(30, 31),
            ),
            types: [createTypeFooBar(31), createTypeFooBar(40)],
            commata: [
              OperatorToken(OperatorTokenType.comma, span: SourceSpan(38, 39)),
            ],
            rightParenthesis: OperatorToken(OperatorTokenType.rparen,
                span: SourceSpan(47, 48)),
          ),
        ],
        parameterCommata: [
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(28, 29)),
        ],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(48, 49)),
        arrow: OperatorToken(
          OperatorTokenType.equalsGreater,
          span: SourceSpan(50, 52),
        ),
        returnType: UnionType(
          leftType: createTypeFooBar(53),
          bar: OperatorToken(OperatorTokenType.bar, span: SourceSpan(61, 62)),
          rightType: createTypeFooBar(63),
        ),
      ),
    },
  );
}

@isTestGroup
void tableTestTypeParser<T extends Type>(
  String description, {
  @required Map<String, T> table,
}) {
  group(description, () {
    forAllMap<String, T>(
      table: table,
      tester: (source, result) =>
          testParser(source, result: result, parser: ParserGrammar.type),
    );
  });
}

Type createTypeFooBar([int offset = 0]) {
  return UserType(
    simpleTypes: [
      SimpleUserType(IdentifierToken(
        'Foo',
        span: SourceSpan.fromStartLength(offset + 0, 3),
      )),
      SimpleUserType(IdentifierToken(
        'Bar',
        span: SourceSpan.fromStartLength(offset + 4, 3),
      )),
    ],
    dots: [
      OperatorToken(
        OperatorTokenType.dot,
        span: SourceSpan.fromStartLength(offset + 3, 1),
      ),
    ],
  );
}
