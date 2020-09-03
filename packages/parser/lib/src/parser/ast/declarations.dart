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
    @required FunKeywordToken funKeyword,
    @required IdentifierToken name,
    @required OperatorToken leftParenthesis,
    @Default(<ValueParameter>[]) List<ValueParameter> valueParameters,
    @Default(<OperatorToken>[]) List<OperatorToken> valueParameterCommata,
    @required OperatorToken rightParenthesis,
    OperatorToken colon,
    Type returnType,
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
abstract class ValueParameter extends AstNode implements _$ValueParameter {
  const factory ValueParameter({
    @required IdentifierToken name,
    @required OperatorToken colon,
    @required Type type,
    OperatorToken equals,
    Expression defaultValue,
  }) = _ValueParameter;
  const ValueParameter._();

  @override
  Iterable<SyntacticEntity> get children => [
        name,
        colon,
        type,
        if (equals != null) equals,
        if (defaultValue != null) defaultValue,
      ];
}
