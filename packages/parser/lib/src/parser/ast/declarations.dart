import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:parser/src/utils.dart';

import '../../lexer/lexer.dart';
import '../../syntactic_entity.dart';
import 'expressions/expression.dart';
import 'node.dart';
import 'statements.dart';
import 'types.dart';

part 'declarations.freezed.dart';

abstract class Declaration extends AstNode {
  const Declaration();
}

@freezed
abstract class FunctionDeclaration extends Declaration
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

@freezed
abstract class PropertyDeclaration extends Declaration
    implements _$PropertyDeclaration {
  const factory PropertyDeclaration({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required LetKeywordToken letKeyword,
    MutKeywordToken mutKeyword,
    @required IdentifierToken name,
    @required OperatorToken colon,
    @required Type type,
    OperatorToken equals,
    Expression initializer,
    @Default(<PropertyAccessor>[]) List<PropertyAccessor> accessors,
  }) = _PropertyDeclaration;
  const PropertyDeclaration._();

  @override
  Iterable<SyntacticEntity> get children => [
        ...modifiers,
        letKeyword,
        if (mutKeyword != null) mutKeyword,
        name,
        colon,
        type,
        if (equals != null) equals,
        if (initializer != null) initializer,
        ...accessors,
      ];
}

@freezed
abstract class PropertyAccessor extends AstNode implements _$PropertyAccessor {
  const factory PropertyAccessor.getter({
    @required GetKeywordToken keyword,
    OperatorToken colon,
    Type returnType,
    Block body,
  }) = GetterPropertyAccessor;
  const factory PropertyAccessor.setter({
    @required SetKeywordToken keyword,
    OperatorToken leftParenthesis,
    // May not have a default value.
    ValueParameter valueParameter,
    OperatorToken valueParameterComma,
    OperatorToken rightParenthesis,
    Block body,
  }) = SetterPropertyAccessor;
  const PropertyAccessor._();

  @override
  Iterable<SyntacticEntity> get children => when(
        getter: (keyword, colon, returnType, _) => [
          keyword,
          if (colon != null) colon,
          if (returnType != null) returnType,
          if (body != null) body,
        ],
        setter: (keyword, leftParenthesis, valueParameter, valueParameterComma,
                rightParenthesis, _) =>
            [
          keyword,
          if (leftParenthesis != null) leftParenthesis,
          if (valueParameter != null) valueParameter,
          if (valueParameterComma != null) valueParameterComma,
          if (rightParenthesis != null) rightParenthesis,
          if (body != null) body,
        ],
      );
}
