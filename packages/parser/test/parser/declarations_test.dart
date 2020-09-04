import 'package:meta/meta.dart';
import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/declarations.dart';
import 'package:parser/src/parser/ast/expressions/expression.dart';
import 'package:parser/src/parser/ast/statements.dart';
import 'package:parser/src/parser/ast/types.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:test/test.dart';

import 'statements_test.dart';
import 'types_test.dart';
import 'utils.dart';

void main() {
  setUpAll(ParserGrammar.init);

  tableTestDeclarationParser<FunctionDeclaration>(
    'FunctionDeclaration',
    table: {
      'fun foo(): Foo.Bar {}': FunctionDeclaration(
        funKeyword: KeywordToken.fun(span: SourceSpan(0, 3)) as FunKeywordToken,
        name: IdentifierToken('foo', span: SourceSpan(4, 7)),
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(7, 8)),
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(8, 9)),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(9, 10)),
        returnType: createTypeFooBar(11),
        body: Block(
          leftBrace: OperatorToken(
            OperatorTokenType.lcurl,
            span: SourceSpan(19, 20),
          ),
          rightBrace: OperatorToken(
            OperatorTokenType.rcurl,
            span: SourceSpan(20, 21),
          ),
        ),
      ),
      'external fun foo(bar: Foo.Bar, baz: Foo.Bar = defaultBaz): Foo.Bar':
          FunctionDeclaration(
        modifiers: [ModifierToken.external(span: SourceSpan(0, 8))],
        funKeyword:
            KeywordToken.fun(span: SourceSpan(9, 12)) as FunKeywordToken,
        name: IdentifierToken('foo', span: SourceSpan(13, 16)),
        leftParenthesis: OperatorToken(
          OperatorTokenType.lparen,
          span: SourceSpan(16, 17),
        ),
        valueParameters: [
          ValueParameter(
            name: IdentifierToken('bar', span: SourceSpan(17, 20)),
            colon: OperatorToken(
              OperatorTokenType.colon,
              span: SourceSpan(20, 21),
            ),
            type: createTypeFooBar(22),
          ),
          ValueParameter(
            name: IdentifierToken('baz', span: SourceSpan(31, 34)),
            colon: OperatorToken(
              OperatorTokenType.colon,
              span: SourceSpan(34, 35),
            ),
            type: createTypeFooBar(36),
            equals: OperatorToken(
              OperatorTokenType.equals,
              span: SourceSpan(44, 45),
            ),
            defaultValue: Identifier(
              IdentifierToken('defaultBaz', span: SourceSpan(46, 56)),
            ),
          ),
        ],
        valueParameterCommata: [
          OperatorToken(OperatorTokenType.comma, span: SourceSpan(29, 30)),
        ],
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(56, 57)),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(57, 58)),
        returnType: createTypeFooBar(59),
      ),
    },
  );

  tableTestDeclarationParser<PropertyDeclaration>(
    'PropertyDeclaration',
    table: {
      'let foo: Foo.Bar': PropertyDeclaration(
        letKeyword: KeywordToken.let(span: SourceSpan(0, 3)) as LetKeywordToken,
        name: IdentifierToken('foo', span: SourceSpan(4, 7)),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(7, 8)),
        type: createTypeFooBar(9),
      ),
      'external let mut bar: Foo.Bar': PropertyDeclaration(
        modifiers: [ModifierToken.external(span: SourceSpan(0, 8))],
        letKeyword:
            KeywordToken.let(span: SourceSpan(9, 12)) as LetKeywordToken,
        mutKeyword:
            KeywordToken.mut(span: SourceSpan(13, 16)) as MutKeywordToken,
        name: IdentifierToken('bar', span: SourceSpan(17, 20)),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(20, 21)),
        type: createTypeFooBar(22),
      ),
      'let mut foo: Foo.Bar = 123\n'
          '  get\n'
          '  get {123}\n'
          '  get: Foo.Bar {123}\n'
          '  set\n'
          '  set {}\n'
          '  set(value: Foo.Bar,) {}': PropertyDeclaration(
        letKeyword: KeywordToken.let(span: SourceSpan(0, 3)) as LetKeywordToken,
        mutKeyword: KeywordToken.mut(span: SourceSpan(4, 7)) as MutKeywordToken,
        name: IdentifierToken('foo', span: SourceSpan(8, 11)),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(11, 12)),
        type: createTypeFooBar(13),
        equals:
            OperatorToken(OperatorTokenType.equals, span: SourceSpan(21, 22)),
        initializer:
            Literal<int>(IntegerLiteralToken(123, span: SourceSpan(23, 26))),
        accessors: [
          PropertyAccessor.getter(
            keyword:
                KeywordToken.get(span: SourceSpan(29, 32)) as GetKeywordToken,
          ),
          PropertyAccessor.getter(
            keyword:
                KeywordToken.get(span: SourceSpan(35, 38)) as GetKeywordToken,
            body: Block(
              leftBrace: OperatorToken(
                OperatorTokenType.lcurl,
                span: SourceSpan(39, 40),
              ),
              statements: [createStatement123(40)],
              rightBrace: OperatorToken(
                OperatorTokenType.rcurl,
                span: SourceSpan(43, 44),
              ),
            ),
          ),
          PropertyAccessor.getter(
            keyword:
                KeywordToken.get(span: SourceSpan(47, 50)) as GetKeywordToken,
            colon: OperatorToken(
              OperatorTokenType.colon,
              span: SourceSpan(50, 51),
            ),
            returnType: createTypeFooBar(52),
            body: Block(
              leftBrace: OperatorToken(
                OperatorTokenType.lcurl,
                span: SourceSpan(60, 61),
              ),
              statements: [createStatement123(61)],
              rightBrace: OperatorToken(
                OperatorTokenType.rcurl,
                span: SourceSpan(64, 65),
              ),
            ),
          ),
          PropertyAccessor.setter(
            keyword:
                KeywordToken.set(span: SourceSpan(68, 71)) as SetKeywordToken,
          ),
          PropertyAccessor.setter(
            keyword:
                KeywordToken.set(span: SourceSpan(74, 77)) as SetKeywordToken,
            body: Block(
              leftBrace: OperatorToken(
                OperatorTokenType.lcurl,
                span: SourceSpan(78, 79),
              ),
              rightBrace: OperatorToken(
                OperatorTokenType.rcurl,
                span: SourceSpan(79, 80),
              ),
            ),
          ),
          PropertyAccessor.setter(
            keyword:
                KeywordToken.set(span: SourceSpan(83, 86)) as SetKeywordToken,
            leftParenthesis: OperatorToken(
              OperatorTokenType.lparen,
              span: SourceSpan(86, 87),
            ),
            valueParameter: ValueParameter(
              name: IdentifierToken('value', span: SourceSpan(87, 92)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(92, 93),
              ),
              type: createTypeFooBar(94),
            ),
            valueParameterComma: OperatorToken(
              OperatorTokenType.comma,
              span: SourceSpan(101, 102),
            ),
            rightParenthesis: OperatorToken(
              OperatorTokenType.rparen,
              span: SourceSpan(102, 103),
            ),
            body: Block(
              leftBrace: OperatorToken(
                OperatorTokenType.lcurl,
                span: SourceSpan(104, 105),
              ),
              rightBrace: OperatorToken(
                OperatorTokenType.rcurl,
                span: SourceSpan(105, 106),
              ),
            ),
          ),
        ],
      ),
    },
  );

  tableTestDeclarationParser<ClassDeclaration>(
    'ClassDeclaration',
    table: {
      'class Foo': ClassDeclaration(
        classKeyword:
            KeywordToken.class_(span: SourceSpan(0, 5)) as ClassKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(6, 9)),
      ),
      'const class Foo {}': ClassDeclaration(
        modifiers: [ModifierToken.const_(span: SourceSpan(0, 5))],
        classKeyword:
            KeywordToken.class_(span: SourceSpan(6, 11)) as ClassKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(12, 15)),
        body: ClassBody(
          leftBrace: OperatorToken(
            OperatorTokenType.lcurl,
            span: SourceSpan(16, 17),
          ),
          rightBrace: OperatorToken(
            OperatorTokenType.rcurl,
            span: SourceSpan(17, 18),
          ),
        ),
      ),
      'abstract class Foo: Bar(baz = 1, false) {\n'
          '  abstract let foo: Int\n'
          '  fun bar() {}\n'
          '}': ClassDeclaration(
        modifiers: [ModifierToken.abstract(span: SourceSpan(0, 8))],
        classKeyword:
            KeywordToken.class_(span: SourceSpan(9, 14)) as ClassKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(15, 18)),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(18, 19)),
        parentConstructorInvocation: ConstructorInvocation(
          type: UserType(simpleTypes: [
            SimpleUserType(IdentifierToken('Bar', span: SourceSpan(20, 23))),
          ]),
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(23, 24)),
          arguments: [
            Argument(
              name: IdentifierToken('baz', span: SourceSpan(24, 27)),
              equals: OperatorToken(
                OperatorTokenType.equals,
                span: SourceSpan(28, 29),
              ),
              expression: Literal<int>(
                IntegerLiteralToken(1, span: SourceSpan(30, 31)),
              ),
            ),
            Argument(
              expression: Literal<bool>(
                BooleanLiteralToken(false, span: SourceSpan(33, 38)),
              ),
            ),
          ],
          argumentCommata: [
            OperatorToken(OperatorTokenType.comma, span: SourceSpan(31, 32)),
          ],
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(38, 39)),
        ),
        body: ClassBody(
          leftBrace: OperatorToken(
            OperatorTokenType.lcurl,
            span: SourceSpan(40, 41),
          ),
          declarations: [
            PropertyDeclaration(
              modifiers: [ModifierToken.abstract(span: SourceSpan(44, 52))],
              letKeyword:
                  KeywordToken.let(span: SourceSpan(53, 56)) as LetKeywordToken,
              name: IdentifierToken('foo', span: SourceSpan(57, 60)),
              colon: OperatorToken(OperatorTokenType.colon,
                  span: SourceSpan(60, 61)),
              type: UserType(simpleTypes: [
                SimpleUserType(
                  IdentifierToken('Int', span: SourceSpan(62, 65)),
                ),
              ]),
            ),
            FunctionDeclaration(
              funKeyword:
                  KeywordToken.fun(span: SourceSpan(68, 71)) as FunKeywordToken,
              name: IdentifierToken('bar', span: SourceSpan(72, 75)),
              leftParenthesis: OperatorToken(
                OperatorTokenType.lparen,
                span: SourceSpan(75, 76),
              ),
              rightParenthesis: OperatorToken(
                OperatorTokenType.rparen,
                span: SourceSpan(76, 77),
              ),
              body: Block(
                leftBrace: OperatorToken(
                  OperatorTokenType.lcurl,
                  span: SourceSpan(78, 79),
                ),
                rightBrace: OperatorToken(
                  OperatorTokenType.rcurl,
                  span: SourceSpan(79, 80),
                ),
              ),
            ),
          ],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(81, 82)),
        ),
      ),
    },
  );
}

@isTestGroup
void tableTestDeclarationParser<D extends Declaration>(
  String description, {
  @required Map<String, D> table,
}) {
  group(description, () {
    forAllMap<String, D>(
      table: table,
      tester: (source, result) =>
          testParser(source, result: result, parser: ParserGrammar.declaration),
    );
  });
}
