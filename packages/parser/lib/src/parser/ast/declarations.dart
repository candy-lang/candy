import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:parser/src/utils.dart';

import '../../lexer/lexer.dart';
import '../../syntactic_entity.dart';
import 'expressions/expression.dart';
import 'node.dart';
import 'statements.dart';
import 'types.dart';

part 'declarations.freezed.dart';

@freezed
abstract class FunctionDeclaration extends AstNode
    implements _$FunctionDeclaration {
  const factory FunctionDeclaration({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required KeywordToken funKeyword,
    @required IdentifierToken name,
    @required OperatorToken leftParenthesis,
    @Default(<FunctionValueParameter>[])
        List<FunctionValueParameter> valueParameters,
    @Default(<OperatorToken>[]) List<OperatorToken> valueParameterCommata,
    @required OperatorToken rightParenthesis,
    @required OperatorToken colon,
    @required Type returnType,
    Block body,
  }) = _FunctionDeclaration;
  const FunctionDeclaration._();

  @override
  Iterable<SyntacticEntity> get children => [
        ...modifiers,
        funKeyword,
        name,
        leftParenthesis,
        ...interleave(valueParameters, valueParameterCommata),
        rightParenthesis,
        colon,
        returnType,
        if (body != null) body,
      ];
}

@freezed
abstract class FunctionValueParameter extends AstNode
    implements _$FunctionValueParameter {
  const factory FunctionValueParameter({
    @required IdentifierToken name,
    @required OperatorToken colon,
    @required Type type,
    OperatorToken equals,
    Expression defaultValue,
  }) = _FunctionValueParameter;
  const FunctionValueParameter._();

  @override
  Iterable<SyntacticEntity> get children => [
        name,
        colon,
        type,
        if (equals != null) equals,
        if (defaultValue != null) defaultValue,
      ];
}
