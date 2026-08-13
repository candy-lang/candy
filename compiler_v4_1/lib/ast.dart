import 'package:supernova/supernova.dart';

import 'json_converters.dart';
import 'lexer/token.dart';
import 'position.dart';

part 'ast.freezed.dart';
part 'ast.g.dart';

/// The result of parsing a syntax element that is required, but might be
/// missing or malformed.
typedef AstErrorOr<T> = Option<T>;

enum AstVisibility { private, package, public }

@freezed
sealed class Ast with _$Ast {
  const factory Ast({
    required List<AstUse> uses,
    required List<AstDeclaration> declarations,
  }) = _Ast;

  factory Ast.fromJson(Map<String, dynamic> json) => _$AstFromJson(json);
  const Ast._();
}

@freezed
sealed class AstUse with _$AstUse {
  const factory AstUse({
    required Token useKeyword,
    required List<Token> path,
  }) = _AstUse;

  factory AstUse.fromJson(Map<String, dynamic> json) => _$AstUseFromJson(json);
  const AstUse._();
}

// Declarations

@freezed
sealed class AstDeclaration with _$AstDeclaration {
  const factory AstDeclaration.struct(AstStruct struct) = AstDeclaration_Struct;
  const factory AstDeclaration.enum_(AstEnum enum_) = AstDeclaration_Enum;
  const factory AstDeclaration.trait(AstTrait trait) = AstDeclaration_Trait;
  const factory AstDeclaration.impl(AstImpl impl) = AstDeclaration_Impl;
  const factory AstDeclaration.assignment(AstAssignment assignment) =
      AstDeclaration_Assignment;
  const factory AstDeclaration.function(AstFunction function) =
      AstDeclaration_Function;

  factory AstDeclaration.fromJson(Map<String, dynamic> json) =>
      _$AstDeclarationFromJson(json);
  const AstDeclaration._();
}

@freezed
sealed class AstStruct with _$AstStruct {
  const factory AstStruct({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required AstStructKind kind,
  }) = _AstStruct;

  factory AstStruct.fromJson(Map<String, dynamic> json) =>
      _$AstStructFromJson(json);
  const AstStruct._();
}

@freezed
sealed class AstStructKind with _$AstStructKind {
  const factory AstStructKind.builtin() = AstStructKind_Builtin;
  const factory AstStructKind.userDefined({
    required List<AstStructField> fields,
  }) = AstStructKind_UserDefined;

  factory AstStructKind.fromJson(Map<String, dynamic> json) =>
      _$AstStructKindFromJson(json);
  const AstStructKind._();
}

@freezed
sealed class AstStructField with _$AstStructField {
  const factory AstStructField({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required AstErrorOr<AstType> type,
  }) = _AstStructField;

  factory AstStructField.fromJson(Map<String, dynamic> json) =>
      _$AstStructFieldFromJson(json);
  const AstStructField._();
}

@freezed
sealed class AstEnum with _$AstEnum {
  const factory AstEnum({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required List<AstEnumVariant> variants,
  }) = _AstEnum;

  factory AstEnum.fromJson(Map<String, dynamic> json) =>
      _$AstEnumFromJson(json);
  const AstEnum._();
}

@freezed
sealed class AstEnumVariant with _$AstEnumVariant {
  const factory AstEnumVariant({
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? type,
  }) = _AstEnumVariant;

  factory AstEnumVariant.fromJson(Map<String, dynamic> json) =>
      _$AstEnumVariantFromJson(json);
  const AstEnumVariant._();
}

@freezed
sealed class AstTrait with _$AstTrait {
  const factory AstTrait({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required List<AstFunction> functions,
  }) = _AstTrait;

  factory AstTrait.fromJson(Map<String, dynamic> json) =>
      _$AstTraitFromJson(json);
  const AstTrait._();
}

@freezed
sealed class AstImpl with _$AstImpl {
  const factory AstImpl({
    required Token implKeyword,
    required AstTypeParameters? typeParameters,
    @_AstTypeOptionConverter() required AstErrorOr<AstType> type,
    @_AstTypeOptionConverter() required AstErrorOr<AstType> trait,
    required List<AstFunction> functions,
  }) = _AstImpl;

  factory AstImpl.fromJson(Map<String, dynamic> json) =>
      _$AstImplFromJson(json);
  const AstImpl._();
}

@freezed
sealed class AstAssignment with _$AstAssignment {
  const factory AstAssignment({
    @Default(AstVisibility.private) AstVisibility visibility,
    required SourceSpan displaySpan,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? type,
    @_AstExpressionOptionConverter() required AstErrorOr<AstExpression> value,
  }) = _AstAssignment;

  factory AstAssignment.fromJson(Map<String, dynamic> json) =>
      _$AstAssignmentFromJson(json);
  const AstAssignment._();
}

@freezed
sealed class AstFunction with _$AstFunction {
  const factory AstFunction({
    @Default(AstVisibility.private) AstVisibility visibility,
    required SourceSpan displaySpan,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    @_AstValueParametersOptionConverter()
    required AstErrorOr<AstValueParameters> valueParameters,
    required AstType? returnType,
    required AstBody? body,
  }) = _AstFunction;

  factory AstFunction.fromJson(Map<String, dynamic> json) =>
      _$AstFunctionFromJson(json);
  const AstFunction._();
}

@freezed
sealed class AstValueParameters with _$AstValueParameters implements ToJsonMap {
  const factory AstValueParameters({
    required List<AstValueParameter> valueParameters,
  }) = _AstValueParameters;

  factory AstValueParameters.fromJson(Map<String, dynamic> json) =>
      _$AstValueParametersFromJson(json);
  const AstValueParameters._();
}

@freezed
sealed class AstValueParameter with _$AstValueParameter {
  const factory AstValueParameter({
    @_TokenOptionConverter() required AstErrorOr<Token> name,

    /// `null` when the parameter has no annotation, which only lambdas allow.
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? type,
  }) = _AstValueParameter;

  factory AstValueParameter.fromJson(Map<String, dynamic> json) =>
      _$AstValueParameterFromJson(json);
  const AstValueParameter._();
}

@freezed
sealed class AstTypeParameters with _$AstTypeParameters {
  const factory AstTypeParameters({
    required List<AstTypeParameter> parameters,
  }) = _AstTypeParameters;

  factory AstTypeParameters.fromJson(Map<String, dynamic> json) =>
      _$AstTypeParametersFromJson(json);
  const AstTypeParameters._();
}

@freezed
sealed class AstTypeParameter with _$AstTypeParameter {
  const factory AstTypeParameter({
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? upperBound,
  }) = _AstTypeParameter;

  factory AstTypeParameter.fromJson(Map<String, dynamic> json) =>
      _$AstTypeParameterFromJson(json);
  const AstTypeParameter._();
}

// Types

@freezed
sealed class AstType with _$AstType implements ToJsonMap {
  const factory AstType.named({
    required Token name,
    required AstTypeArguments? typeArguments,
  }) = AstType_Named;
  const factory AstType.function({
    required List<AstType> parameterTypes,
    @_AstTypeOptionConverter() required AstErrorOr<AstType> returnType,
  }) = AstType_Function;

  factory AstType.fromJson(Map<String, dynamic> json) =>
      _$AstTypeFromJson(json);
  const AstType._();
}

@freezed
sealed class AstTypeArguments with _$AstTypeArguments {
  const factory AstTypeArguments({required List<AstType> arguments}) =
      _AstTypeArguments;

  factory AstTypeArguments.fromJson(Map<String, dynamic> json) =>
      _$AstTypeArgumentsFromJson(json);
  const AstTypeArguments._();
}

// Expressions

@freezed
sealed class AstExpression with _$AstExpression implements ToJsonMap {
  const factory AstExpression.int({required int value, required Token token}) =
      AstExpression_Int;
  const factory AstExpression.identifier(Token identifier) =
      AstExpression_Identifier;
  const factory AstExpression.text({required List<AstTextPart> parts}) =
      AstExpression_Text;
  const factory AstExpression.parenthesized(AstParenthesized parenthesized) =
      AstExpression_Parenthesized;
  // The parameter can't be named `call` since that would collide with the
  // generated `copyWith.call(…)`.
  const factory AstExpression.call(AstCall call_) = AstExpression_Call;
  const factory AstExpression.navigation(AstNavigation navigation) =
      AstExpression_Navigation;
  const factory AstExpression.lambda(AstLambda lambda) = AstExpression_Lambda;
  const factory AstExpression.body(AstBody body) = AstExpression_Body;
  const factory AstExpression.switch_(AstSwitch switch_) = AstExpression_Switch;

  factory AstExpression.fromJson(Map<String, dynamic> json) =>
      _$AstExpressionFromJson(json);
  const AstExpression._();
}

@freezed
sealed class AstTextPart with _$AstTextPart {
  const factory AstTextPart.text({
    required Token token,
    required String value,
  }) = AstTextPart_Text;
  const factory AstTextPart.interpolation({
    @_AstExpressionOptionConverter()
    required AstErrorOr<AstExpression> expression,
  }) = AstTextPart_Interpolation;

  factory AstTextPart.fromJson(Map<String, dynamic> json) =>
      _$AstTextPartFromJson(json);
  const AstTextPart._();
}

@freezed
sealed class AstParenthesized with _$AstParenthesized {
  const factory AstParenthesized({
    @_AstExpressionOptionConverter() required AstErrorOr<AstExpression> inner,
  }) = _AstParenthesized;

  factory AstParenthesized.fromJson(Map<String, dynamic> json) =>
      _$AstParenthesizedFromJson(json);
  const AstParenthesized._();
}

@freezed
sealed class AstCall with _$AstCall {
  const factory AstCall({
    required AstExpression receiver,
    required AstTypeArguments? typeArguments,
    @_AstArgumentsOptionConverter() required AstErrorOr<AstArguments> arguments,
  }) = _AstCall;

  factory AstCall.fromJson(Map<String, dynamic> json) =>
      _$AstCallFromJson(json);
  const AstCall._();
}

@freezed
sealed class AstArguments with _$AstArguments implements ToJsonMap {
  const factory AstArguments({
    required SourceSpan openingParenthesisSpan,
    required List<AstExpression> arguments,
  }) = _AstArguments;

  factory AstArguments.fromJson(Map<String, dynamic> json) =>
      _$AstArgumentsFromJson(json);
  const AstArguments._();
}

extension AstErrorOrAstArguments on AstErrorOr<AstArguments> {
  List<AstExpression> get argumentsOrDefault =>
      toNullable()?.arguments ?? const [];
}

@freezed
sealed class AstNavigation with _$AstNavigation {
  const factory AstNavigation({
    required AstExpression receiver,
    @_TokenOptionConverter() required AstErrorOr<Token> key,
  }) = _AstNavigation;

  factory AstNavigation.fromJson(Map<String, dynamic> json) =>
      _$AstNavigationFromJson(json);
  const AstNavigation._();
}

@freezed
sealed class AstLambda with _$AstLambda {
  const factory AstLambda({
    required AstValueParameters parameters,
    required AstBody body,
  }) = _AstLambda;

  factory AstLambda.fromJson(Map<String, dynamic> json) =>
      _$AstLambdaFromJson(json);
  const AstLambda._();
}

@freezed
sealed class AstBody with _$AstBody {
  const factory AstBody({required List<AstStatement> statements}) = _AstBody;

  factory AstBody.fromJson(Map<String, dynamic> json) =>
      _$AstBodyFromJson(json);
  const AstBody._();
}

@freezed
sealed class AstStatement with _$AstStatement {
  const factory AstStatement.assignment(AstAssignment assignment) =
      AstStatement_Assignment;
  const factory AstStatement.expression(AstExpression expression) =
      AstStatement_Expression;

  factory AstStatement.fromJson(Map<String, dynamic> json) =>
      _$AstStatementFromJson(json);
  const AstStatement._();
}

@freezed
sealed class AstSwitch with _$AstSwitch {
  const factory AstSwitch({
    @_AstExpressionOptionConverter() required AstErrorOr<AstExpression> value,
    required List<AstSwitchCase> cases,
  }) = _AstSwitch;

  factory AstSwitch.fromJson(Map<String, dynamic> json) =>
      _$AstSwitchFromJson(json);
  const AstSwitch._();
}

@freezed
sealed class AstSwitchCase with _$AstSwitchCase {
  const factory AstSwitchCase({
    @_TokenOptionConverter() required AstErrorOr<Token> variant,
    @_TokenOptionConverter() required AstErrorOr<Token>? valueName,
    @_AstExpressionOptionConverter()
    required AstErrorOr<AstExpression> expression,
  }) = _AstSwitchCase;

  factory AstSwitchCase.fromJson(Map<String, dynamic> json) =>
      _$AstSwitchCaseFromJson(json);
  const AstSwitchCase._();
}

class _TokenOptionConverter
    extends OptionConverter<Token, Map<String, dynamic>> {
  const _TokenOptionConverter() : super(Token.fromJson);
}

class _AstTypeOptionConverter
    extends OptionConverter<AstType, Map<String, dynamic>> {
  const _AstTypeOptionConverter() : super(AstType.fromJson);
}

class _AstExpressionOptionConverter
    extends OptionConverter<AstExpression, Map<String, dynamic>> {
  const _AstExpressionOptionConverter() : super(AstExpression.fromJson);
}

class _AstArgumentsOptionConverter
    extends OptionConverter<AstArguments, Map<String, dynamic>> {
  const _AstArgumentsOptionConverter() : super(AstArguments.fromJson);
}

class _AstValueParametersOptionConverter
    extends OptionConverter<AstValueParameters, Map<String, dynamic>> {
  const _AstValueParametersOptionConverter()
    : super(AstValueParameters.fromJson);
}
