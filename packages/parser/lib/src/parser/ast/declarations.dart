import 'package:dartx/dartx.dart';
import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/lexer.dart';
import '../../syntactic_entity.dart';
import '../../utils.dart';
import 'expressions/expressions.dart';
import 'node.dart';
import 'statements.dart';
import 'types.dart';

part 'declarations.freezed.dart';

abstract class Declaration extends AstNode {
  const Declaration();
}

@freezed
abstract class ModuleDeclaration extends Declaration
    implements _$ModuleDeclaration {
  const factory ModuleDeclaration({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required ModuleKeywordToken moduleKeyword,
    @required IdentifierToken name,
    @required BlockDeclarationBody body,
  }) = _ModuleDeclaration;
  const ModuleDeclaration._();

  @override
  Iterable<SyntacticEntity> get children =>
      [...modifiers, moduleKeyword, name, body];
}

@freezed
abstract class TraitDeclaration extends Declaration
    implements _$TraitDeclaration {
  const factory TraitDeclaration({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required TraitKeywordToken traitKeyword,
    @required IdentifierToken name,
    TypeParameters typeParameters,
    OperatorToken colon,
    Type bound,
    BlockDeclarationBody body,
  }) = _TraitDeclaration;
  const TraitDeclaration._();

  @override
  Iterable<SyntacticEntity> get children => [
        ...modifiers,
        traitKeyword,
        name,
        if (typeParameters != null) typeParameters,
        if (colon != null) colon,
        if (bound != null) bound,
        if (body != null) body,
      ];
}

@freezed
abstract class ImplDeclaration extends Declaration
    implements _$ImplDeclaration {
  const factory ImplDeclaration({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required ImplKeywordToken implKeyword,
    TypeParameters typeParameters,
    @required Type type,
    OperatorToken colon,
    Type trait,
    BlockDeclarationBody body,
  }) = _ImplDeclaration;
  const ImplDeclaration._();

  @override
  Iterable<SyntacticEntity> get children => [
        ...modifiers,
        implKeyword,
        if (typeParameters != null) typeParameters,
        type,
        if (colon != null) colon,
        if (trait != null) trait,
        if (body != null) body,
      ];
}

@freezed
abstract class ClassDeclaration extends Declaration
    implements _$ClassDeclaration {
  const factory ClassDeclaration({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required ClassKeywordToken classKeyword,
    @required IdentifierToken name,
    TypeParameters typeParameters,
    BlockDeclarationBody body,
  }) = _ClassDeclaration;
  const ClassDeclaration._();

  @override
  Iterable<SyntacticEntity> get children => [
        ...modifiers,
        classKeyword,
        name,
        if (typeParameters != null) typeParameters,
        if (body != null) body,
      ];
}

@freezed
abstract class BlockDeclarationBody extends AstNode
    implements _$BlockDeclarationBody {
  const factory BlockDeclarationBody({
    @required OperatorToken leftBrace,
    @Default(<Declaration>[]) List<Declaration> declarations,
    @required OperatorToken rightBrace,
  }) = _BlockDeclarationBody;
  const BlockDeclarationBody._();

  @override
  Iterable<SyntacticEntity> get children =>
      [leftBrace, ...declarations, rightBrace];
}

@freezed
abstract class ConstructorCall extends AstNode implements _$ConstructorCall {
  const factory ConstructorCall({
    @required UserType type,
    @required OperatorToken leftParenthesis,
    @required List<Argument> arguments,
    @required List<OperatorToken> argumentCommata,
    @required OperatorToken rightParenthesis,
  }) = _ConstructorCall;
  const ConstructorCall._();

  @override
  Iterable<SyntacticEntity> get children => [
        type,
        leftParenthesis,
        ...interleave(arguments, argumentCommata),
        rightParenthesis,
      ];
}

@freezed
abstract class FunctionDeclaration extends Declaration
    implements _$FunctionDeclaration {
  const factory FunctionDeclaration({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required FunKeywordToken funKeyword,
    @required IdentifierToken name,
    TypeParameters typeParameters,
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
        if (typeParameters != null) typeParameters,
        leftParenthesis,
        ...interleave(valueParameters, valueParameterCommata),
        rightParenthesis,
        if (colon != null) colon,
        if (returnType != null) returnType,
        if (body != null) body,
      ];
}

@freezed
abstract class ValueParameter extends AstNode implements _$ValueParameter {
  const factory ValueParameter({
    @required IdentifierToken name,
    OperatorToken colon,
    Type type,
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

  GetterPropertyAccessor get getter =>
      accessors.whereType<GetterPropertyAccessor>().firstOrNull;
  SetterPropertyAccessor get setter =>
      accessors.whereType<SetterPropertyAccessor>().firstOrNull;
}

@freezed
abstract class PropertyAccessor extends Declaration
    implements _$PropertyAccessor {
  const factory PropertyAccessor.getter({
    @required GetKeywordToken keyword,
    Block body,
  }) = GetterPropertyAccessor;
  const factory PropertyAccessor.setter({
    @required SetKeywordToken keyword,
    Block body,
  }) = SetterPropertyAccessor;
  const PropertyAccessor._();

  @override
  Iterable<SyntacticEntity> get children => when(
        getter: (keyword, _) => [keyword, if (body != null) body],
        setter: (keyword, _) => [keyword, if (body != null) body],
      );
}
