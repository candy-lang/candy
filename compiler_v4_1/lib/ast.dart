import 'package:supernova/supernova.dart';

import 'error.dart';
import 'lexer/token.dart';
import 'position.dart';

part 'ast.freezed.dart';

@freezed
sealed class AstString with _$AstString {
  const factory AstString(String string, {required SourceSpan span}) =
      _AstString;

  // factory AstString.fromJson(Map<String, dynamic> json) =>
  //     _$AstStringFromJson(json);
  const AstString._();
}

// @freezed
// sealed class AstResult<T> with _$AstResult<T> {
//   const factory AstResult({
//     required T? value,
//     required List<CompilerError> errors,
//   }) = _AstResult;

//   // factory AstResult.fromJson(
//   //   Map<String, dynamic> json,
//   //   T Function(Object? json) fromJsonT,
//   // ) => _$AstResultFromJson(json, fromJsonT);
//   const AstResult._();

//   factory AstResult.ok(T value) => AstResult(value: value, errors: []);
//   factory AstResult.error(T? value, CompilerError error) =>
//       AstResult.errors(value, [error]);
//   factory AstResult.errors(T? value, List<CompilerError> errors) {
//     assert(errors.isNotEmpty);
//     return AstResult(value: value, errors: errors);
//   }

//   AstResult<U> map<U>(Mapper<T, U> map) =>
//       AstResult(value: value?.let(map), errors: errors);
// }

typedef AstErrorOr<T> = Option<T>;

typedef Ast = List<AstDeclaration>;

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

  // factory AstDeclaration.fromJson(Map<String, dynamic> json) =>
  //     _$AstDeclarationFromJson(json);
  const AstDeclaration._();
}

@freezed
sealed class AstStruct with _$AstStruct {
  const factory AstStruct({
    required AstResult<AstString> name,
    required AstTypeParameters? typeParameters,
    required AstStructKind kind,
  }) = _AstStruct;

  // factory AstStruct.fromJson(Map<String, dynamic> json) =>
  //     _$AstStructFromJson(json);
  const AstStruct._();
}

@freezed
sealed class AstStructKind with _$AstStructKind {
  const factory AstStructKind.builtin({
    required CompilerError? optionalKeywordError,
  }) = AstStructKind_Builtin;
  const factory AstStructKind.userDefined({
    required CompilerError? openingCurlyBraceError,
    required List<AstStructField> fields,
    required CompilerError? closingCurlyBraceError,
  }) = AstStructKind_UserDefined;

  // factory AstStructKind.fromJson(Map<String, dynamic> json) =>
  //     _$AstStructKindFromJson(json);
  const AstStructKind._();
}

@freezed
sealed class AstStructField with _$AstStructField {
  const factory AstStructField({
    required AstResult<AstString> name,
    required CompilerError? colonError,
    required AstResult<AstType> type,
    required CompilerError? commaError,
  }) = _AstStructField;

  // factory AstStructField.fromJson(Map<String, dynamic> json) =>
  //     _$AstStructFieldFromJson(json);
  const AstStructField._();
}

@freezed
sealed class AstEnum with _$AstEnum {
  const factory AstEnum({
    required AstResult<AstString> name,
    required AstTypeParameters? typeParameters,
    required CompilerError? openingCurlyBraceError,
    required List<AstEnumVariant> variants,
    required CompilerError? closingCurlyBraceError,
  }) = _AstEnum;

  // factory AstEnum.fromJson(Map<String, dynamic> json) =>
  //     _$AstEnumFromJson(json);
  const AstEnum._();
}

@freezed
sealed class AstEnumVariant with _$AstEnumVariant {
  const factory AstEnumVariant({
    required AstResult<AstString> name,
    required AstResult<AstType>? type,
    required CompilerError? commaError,
  }) = _AstEnumVariant;

  // factory AstEnumVariant.fromJson(Map<String, dynamic> json) =>
  //     _$AstEnumVariantFromJson(json);
  const AstEnumVariant._();
}

@freezed
sealed class AstTrait with _$AstTrait {
  const factory AstTrait({
    required AstResult<AstString> name,
    required AstTypeParameters? typeParameters,
    required CompilerError? openingCurlyBraceError,
    required List<AstFunction> functions,
    required CompilerError? closingCurlyBraceError,
  }) = _AstTrait;

  // factory AstTrait.fromJson(Map<String, dynamic> json) =>
  //     _$AstTraitFromJson(json);
  const AstTrait._();
}

@freezed
sealed class AstImpl with _$AstImpl {
  const factory AstImpl({
    required Token implKeyword,
    required AstTypeParameters? typeParameters,
    required AstResult<AstType> type,
    required CompilerError? colonError,
    required AstResult<AstType> trait,
    required CompilerError? openingCurlyBraceError,
    required List<AstFunction> functions,
    required CompilerError? closingCurlyBraceError,
  }) = _AstImpl;

  // factory AstImpl.fromJson(Map<String, dynamic> json) =>
  //     _$AstImplFromJson(json);
  const AstImpl._();
}

@freezed
sealed class AstAssignment with _$AstAssignment {
  const factory AstAssignment({
    required SourceSpan displaySpan,
    required AstResult<Token> name,
    required AstResult<AstType>? type,
    required CompilerError? equalsSignError,
    required AstResult<AstExpression> value,
  }) = _AstAssignment;

  // factory AstAssignment.fromJson(Map<String, dynamic> json) =>
  //     _$AstAssignmentFromJson(json);
  const AstAssignment._();
}

@freezed
sealed class AstFunction with _$AstFunction {
  const factory AstFunction({
    required SourceSpan displaySpan,
    required Token? name,
    required AstTypeParameters? typeParameters,
    required AstValueParameters? valueParameters,
    required AstType? returnType,
    required AstBody? body,
  }) = _AstFunction;

  // factory AstFunction.fromJson(Map<String, dynamic> json) =>
  //     _$AstFunctionFromJson(json);
  const AstFunction._();
}

@freezed
sealed class AstValueParameters with _$AstValueParameters {
  const factory AstValueParameters({
    required List<AstValueParameter> valueParameters,
    required CompilerError? closingParenthesisError,
  }) = _AstParameters;

  // factory AstParameters.fromJson(Map<String, dynamic> json) =>
  //     _$AstParametersFromJson(json);
  const AstValueParameters._();
}

@freezed
sealed class AstValueParameter with _$AstValueParameter {
  const factory AstValueParameter({
    required AstErrorOr<Token> name,
    required AstErrorOr<AstType> type,
  }) = _AstParameter;

  // factory AstParameter.fromJson(Map<String, dynamic> json) =>
  //     _$AstParameterFromJson(json);
  const AstValueParameter._();
}

@freezed
sealed class AstTypeParameters with _$AstTypeParameters {
  const factory AstTypeParameters({
    required List<AstErrorOr<AstTypeParameter>> parameters,
  }) = _AstTypeParameters;

  // factory AstTypeParameters.fromJson(Map<String, dynamic> json) =>
  //     _$AstTypeParametersFromJson(json);
  const AstTypeParameters._();
}

@freezed
sealed class AstTypeParameter with _$AstTypeParameter {
  const factory AstTypeParameter({
    required Token name,
    required AstErrorOr<AstType>? upperBound,
  }) = _AstTypeParameter;

  // factory AstTypeParameter.fromJson(Map<String, dynamic> json) =>
  //     _$AstTypeParameterFromJson(json);
  const AstTypeParameter._();
}

// Types

@freezed
sealed class AstType with _$AstType {
  const factory AstType.named({
    required Token name,
    required AstTypeArguments? typeArguments,
  }) = AstType_Named;
  const factory AstType.function({
    required List<AstType> parameterTypes,
    required AstErrorOr<AstType>? returnType,
  }) = AstType_Function;

  // factory AstType.fromJson(Map<String, dynamic> json) =>
  //     _$AstTypeFromJson(json);
  const AstType._();
}

@freezed
sealed class AstTypeArguments with _$AstTypeArguments {
  const factory AstTypeArguments({required List<AstType> arguments}) =
      _AstTypeArguments;

  // factory AstTypeArguments.fromJson(Map<String, dynamic> json) =>
  //     _$AstTypeArgumentsFromJson(json);
  const AstTypeArguments._();
}

// Expressions

@freezed
sealed class AstExpression with _$AstExpression {
  const factory AstExpression.int({required int value, required Token token}) =
      AstExpression_Int;
  const factory AstExpression.identifier(Token identifier) =
      AstExpression_Identifier;
  const factory AstExpression.text({required List<AstTextPart> parts}) =
      AstExpression_Text;
  const factory AstExpression.parenthesized(AstParenthesized parenthesized) =
      AstExpression_Parenthesized;
  const factory AstExpression.call(AstCall call) = AstExpression_Call;
  const factory AstExpression.navigation(AstNavigation navigation) =
      AstExpression_Navigation;
  const factory AstExpression.lambda(AstLambda lambda) = AstExpression_Lambda;
  const factory AstExpression.body(AstBody body) = AstExpression_Body;
  const factory AstExpression.switch_(AstSwitch switch_) = AstExpression_Switch;

  // factory AstExpression.fromJson(Map<String, dynamic> json) =>
  //     _$AstExpressionFromJson(json);
  const AstExpression._();
}

@freezed
sealed class AstTextPart with _$AstTextPart {
  const factory AstTextPart.text({
    required Token token,
    required String value,
  }) = _AstTextPart_Text;
  const factory AstTextPart.interpolation({
    required AstErrorOr<AstExpression> expression,
  }) = _AstTextPart_Interpolation;

  // factory AstTextPart.fromJson(Map<String, dynamic> json) =>
  //     _$AstTextPartFromJson(json);
  const AstTextPart._();
}

@freezed
sealed class AstParenthesized with _$AstParenthesized {
  const factory AstParenthesized({required AstErrorOr<AstExpression> inner}) =
      _AstParenthesized;

  // factory AstParenthesized.fromJson(Map<String, dynamic> json) =>
  //     _$AstParenthesizedFromJson(json);
  const AstParenthesized._();
}

@freezed
sealed class AstCall with _$AstCall {
  const factory AstCall({
    required AstExpression receiver,
    required AstTypeArguments typeArguments,
    required AstErrorOr<AstArguments> arguments,
  }) = _AstCall;

  // factory AstCall.fromJson(Map<String, dynamic> json) =>
  //     _$AstCallFromJson(json);
  const AstCall._();
}

@freezed
sealed class AstArguments with _$AstArguments {
  const factory AstArguments({
    required SourceSpan openingParenthesisSpan,
    required List<AstArgument> arguments,
    required CompilerError? closingParenthesisError,
  }) = _AstArguments;

  // factory AstArguments.fromJson(Map<String, dynamic> json) =>
  //     _$AstArgumentsFromJson(json);
  const AstArguments._();
}

extension AstErrorOrAstArguments on AstErrorOr<AstArguments> {
  List<AstArgument> get argumentsOrDefault => value?.arguments ?? [];
}

@freezed
sealed class AstArgument with _$AstArgument {
  const factory AstArgument({
    required AstExpression value,
    required CompilerError? commaError,
  }) = _AstArgument;

  // factory AstArgument.fromJson(Map<String, dynamic> json) =>
  //     _$AstArgumentFromJson(json);
  const AstArgument._();
}

@freezed
sealed class AstNavigation with _$AstNavigation {
  const factory AstNavigation({
    required AstExpression receiver,
    required AstErrorOr<AstString> key,
  }) = _AstNavigation;

  // factory AstNavigation.fromJson(Map<String, dynamic> json) =>
  //     _$AstNavigationFromJson(json);
  const AstNavigation._();
}

@freezed
sealed class AstLambda with _$AstLambda {
  const factory AstLambda({
    required AstValueParameters parameters,
    required AstBody body,
  }) = _AstLambda;

  // factory AstLambda.fromJson(Map<String, dynamic> json) =>
  //     _$AstLambdaFromJson(json);
  const AstLambda._();
}

@freezed
sealed class AstBody with _$AstBody {
  const factory AstBody({
    required List<AstStatement> statements,
    required CompilerError? closingCurlyBraceError,
  }) = _AstBody;

  // factory AstBody.fromJson(Map<String, dynamic> json) =>
  //     _$AstBodyFromJson(json);
  const AstBody._();
}

@freezed
sealed class AstStatement with _$AstStatement {
  const factory AstStatement.assignment(AstAssignment assignment) =
      AstStatement_Assignment;
  const factory AstStatement.expression(AstExpression expression) =
      AstStatement_Expression;

  // factory AstStatement.fromJson(Map<String, dynamic> json) =>
  //     _$AstStatementFromJson(json);
  const AstStatement._();
}

@freezed
sealed class AstSwitch with _$AstSwitch {
  const factory AstSwitch({
    required AstErrorOr<AstExpression> value,
    required CompilerError? openingCurlyBraceError,
    required List<AstSwitchCase> cases,
    required CompilerError? closingCurlyBraceError,
  }) = _AstSwitch;

  // factory AstSwitch.fromJson(Map<String, dynamic> json) =>
  //     _$AstSwitchFromJson(json);
  const AstSwitch._();
}

@freezed
sealed class AstSwitchCase with _$AstSwitchCase {
  const factory AstSwitchCase({
    required AstResult<AstString> variant,
    required (AstResult<AstString>, CompilerError?)? valueName,
    required CompilerError? arrowError,
    required AstResult<AstExpression> expression,
    required CompilerError? commaError,
  }) = _AstSwitchCase;

  // factory AstSwitchCase.fromJson(Map<String, dynamic> json) =>
  //     _$AstSwitchCaseFromJson(json);
  const AstSwitchCase._();
}
