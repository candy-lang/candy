import 'package:meta/meta.dart';
import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/declarations.dart';
import 'package:parser/src/parser/ast/expressions/expressions.dart';
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
      'external fun foo<T, R: Foo.Bar>()': FunctionDeclaration(
        modifiers: [ModifierToken.external(span: SourceSpan(0, 8))],
        funKeyword:
            KeywordToken.fun(span: SourceSpan(9, 12)) as FunKeywordToken,
        name: IdentifierToken('foo', span: SourceSpan(13, 16)),
        typeParameters: TypeParameters(
          leftAngle:
              OperatorToken(OperatorTokenType.langle, span: SourceSpan(16, 17)),
          parameters: [
            TypeParameter(name: IdentifierToken('T', span: SourceSpan(17, 18))),
            TypeParameter(
              name: IdentifierToken('R', span: SourceSpan(20, 21)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(21, 22),
              ),
              bound: createTypeFooBar(23),
            ),
          ],
          commata: [
            OperatorToken(OperatorTokenType.comma, span: SourceSpan(18, 19)),
          ],
          rightAngle:
              OperatorToken(OperatorTokenType.rangle, span: SourceSpan(30, 31)),
        ),
        leftParenthesis:
            OperatorToken(OperatorTokenType.lparen, span: SourceSpan(31, 32)),
        rightParenthesis:
            OperatorToken(OperatorTokenType.rparen, span: SourceSpan(32, 33)),
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

  tableTestDeclarationParser<TraitDeclaration>(
    'TraitDeclaration',
    table: {
      'trait Foo': TraitDeclaration(
        traitKeyword:
            KeywordToken.trait(span: SourceSpan(0, 5)) as TraitKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(6, 9)),
      ),
      'trait Foo<T: Foo.Bar>': TraitDeclaration(
        traitKeyword:
            KeywordToken.trait(span: SourceSpan(0, 5)) as TraitKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(6, 9)),
        typeParameters: TypeParameters(
          leftAngle:
              OperatorToken(OperatorTokenType.langle, span: SourceSpan(9, 10)),
          parameters: [
            TypeParameter(
              name: IdentifierToken('T', span: SourceSpan(10, 11)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(11, 12),
              ),
              bound: createTypeFooBar(13),
            ),
          ],
          rightAngle:
              OperatorToken(OperatorTokenType.rangle, span: SourceSpan(20, 21)),
        ),
      ),
      'const trait Foo {}': TraitDeclaration(
        modifiers: [ModifierToken.const_(span: SourceSpan(0, 5))],
        traitKeyword:
            KeywordToken.trait(span: SourceSpan(6, 11)) as TraitKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(12, 15)),
        body: BlockDeclarationBody(
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
      'trait Baz<T>: Foo.Bar<T> {\n'
          '  let foo: Foo.Bar\n'
          '}': TraitDeclaration(
        traitKeyword:
            KeywordToken.trait(span: SourceSpan(0, 5)) as TraitKeywordToken,
        name: IdentifierToken('Baz', span: SourceSpan(6, 9)),
        typeParameters: TypeParameters(
          leftAngle:
              OperatorToken(OperatorTokenType.langle, span: SourceSpan(9, 10)),
          parameters: [
            TypeParameter(name: IdentifierToken('T', span: SourceSpan(10, 11))),
          ],
          rightAngle:
              OperatorToken(OperatorTokenType.rangle, span: SourceSpan(11, 12)),
        ),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(12, 13)),
        bound: UserType(
          simpleTypes: [
            SimpleUserType(IdentifierToken('Foo', span: SourceSpan(14, 17))),
            SimpleUserType(IdentifierToken('Bar', span: SourceSpan(18, 21))),
          ],
          dots: [
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(17, 18)),
          ],
          arguments: TypeArguments(
            leftAngle: OperatorToken(
              OperatorTokenType.langle,
              span: SourceSpan(21, 22),
            ),
            arguments: [
              TypeArgument(
                type: UserType(
                  simpleTypes: [
                    SimpleUserType(
                      IdentifierToken('T', span: SourceSpan(22, 23)),
                    ),
                  ],
                ),
              ),
            ],
            rightAngle: OperatorToken(
              OperatorTokenType.rangle,
              span: SourceSpan(23, 24),
            ),
          ),
        ),
        body: BlockDeclarationBody(
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(25, 26)),
          declarations: [
            PropertyDeclaration(
              letKeyword:
                  KeywordToken.let(span: SourceSpan(29, 32)) as LetKeywordToken,
              name: IdentifierToken('foo', span: SourceSpan(33, 36)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(36, 37),
              ),
              type: createTypeFooBar(38),
            ),
          ],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(46, 47)),
        ),
      ),
    },
  );

  tableTestDeclarationParser<ImplDeclaration>(
    'ImplDeclaration',
    table: {
      'impl Baz': ImplDeclaration(
        implKeyword:
            KeywordToken.impl(span: SourceSpan(0, 4)) as ImplKeywordToken,
        type: UserType(
          simpleTypes: [
            SimpleUserType(IdentifierToken('Baz', span: SourceSpan(5, 8))),
          ],
        ),
      ),
      'impl Baz: Foo.Bar': ImplDeclaration(
        implKeyword:
            KeywordToken.impl(span: SourceSpan(0, 4)) as ImplKeywordToken,
        type: UserType(
          simpleTypes: [
            SimpleUserType(IdentifierToken('Baz', span: SourceSpan(5, 8))),
          ],
        ),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(8, 9)),
        trait: createTypeFooBar(10),
      ),
      'impl<T: Foo.Bar> Foo<T>: Bar<T>': ImplDeclaration(
        implKeyword:
            KeywordToken.impl(span: SourceSpan(0, 4)) as ImplKeywordToken,
        typeParameters: TypeParameters(
          leftAngle:
              OperatorToken(OperatorTokenType.langle, span: SourceSpan(4, 5)),
          parameters: [
            TypeParameter(
              name: IdentifierToken('T', span: SourceSpan(5, 6)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(6, 7),
              ),
              bound: createTypeFooBar(8),
            ),
          ],
          rightAngle:
              OperatorToken(OperatorTokenType.rangle, span: SourceSpan(15, 16)),
        ),
        type: UserType(
          simpleTypes: [
            SimpleUserType(IdentifierToken('Foo', span: SourceSpan(17, 20))),
          ],
          arguments: TypeArguments(
            leftAngle: OperatorToken(
              OperatorTokenType.langle,
              span: SourceSpan(20, 21),
            ),
            arguments: [
              TypeArgument(
                type: UserType(
                  simpleTypes: [
                    SimpleUserType(
                      IdentifierToken('T', span: SourceSpan(21, 22)),
                    ),
                  ],
                ),
              ),
            ],
            rightAngle: OperatorToken(
              OperatorTokenType.rangle,
              span: SourceSpan(22, 23),
            ),
          ),
        ),
        colon: OperatorToken(OperatorTokenType.colon, span: SourceSpan(23, 24)),
        trait: UserType(
          simpleTypes: [
            SimpleUserType(IdentifierToken('Bar', span: SourceSpan(25, 28))),
          ],
          arguments: TypeArguments(
            leftAngle: OperatorToken(
              OperatorTokenType.langle,
              span: SourceSpan(28, 29),
            ),
            arguments: [
              TypeArgument(
                type: UserType(
                  simpleTypes: [
                    SimpleUserType(
                      IdentifierToken('T', span: SourceSpan(29, 30)),
                    ),
                  ],
                ),
              ),
            ],
            rightAngle: OperatorToken(
              OperatorTokenType.rangle,
              span: SourceSpan(30, 31),
            ),
          ),
        ),
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
      'class Foo<T: Foo.Bar>': ClassDeclaration(
        classKeyword:
            KeywordToken.class_(span: SourceSpan(0, 5)) as ClassKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(6, 9)),
        typeParameters: TypeParameters(
          leftAngle:
              OperatorToken(OperatorTokenType.langle, span: SourceSpan(9, 10)),
          parameters: [
            TypeParameter(
              name: IdentifierToken('T', span: SourceSpan(10, 11)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(11, 12),
              ),
              bound: createTypeFooBar(13),
            ),
          ],
          rightAngle:
              OperatorToken(OperatorTokenType.rangle, span: SourceSpan(20, 21)),
        ),
      ),
      'const class Foo {}': ClassDeclaration(
        modifiers: [ModifierToken.const_(span: SourceSpan(0, 5))],
        classKeyword:
            KeywordToken.class_(span: SourceSpan(6, 11)) as ClassKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(12, 15)),
        body: BlockDeclarationBody(
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
      'class Foo {\n'
          '  let foo: Int\n'
          '  fun bar() {}\n'
          '}': ClassDeclaration(
        classKeyword:
            KeywordToken.class_(span: SourceSpan(0, 5)) as ClassKeywordToken,
        name: IdentifierToken('Foo', span: SourceSpan(6, 9)),
        body: BlockDeclarationBody(
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(10, 11)),
          declarations: [
            PropertyDeclaration(
              letKeyword:
                  KeywordToken.let(span: SourceSpan(14, 17)) as LetKeywordToken,
              name: IdentifierToken('foo', span: SourceSpan(18, 21)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(21, 22),
              ),
              type: UserType(simpleTypes: [
                SimpleUserType(
                  IdentifierToken('Int', span: SourceSpan(23, 26)),
                ),
              ]),
            ),
            FunctionDeclaration(
              funKeyword:
                  KeywordToken.fun(span: SourceSpan(29, 32)) as FunKeywordToken,
              name: IdentifierToken('bar', span: SourceSpan(33, 36)),
              leftParenthesis: OperatorToken(
                OperatorTokenType.lparen,
                span: SourceSpan(36, 37),
              ),
              rightParenthesis: OperatorToken(
                OperatorTokenType.rparen,
                span: SourceSpan(37, 38),
              ),
              body: Block(
                leftBrace: OperatorToken(
                  OperatorTokenType.lcurl,
                  span: SourceSpan(39, 40),
                ),
                rightBrace: OperatorToken(
                  OperatorTokenType.rcurl,
                  span: SourceSpan(40, 41),
                ),
              ),
            ),
          ],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(42, 43)),
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
