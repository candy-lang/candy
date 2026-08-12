import 'package:supernova/supernova.dart';

import 'lexer/token.dart';
import 'position.dart';

part 'ast.freezed.dart';

/// The result of parsing a syntax element that is required, but might be
/// missing or malformed.
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

  const AstDeclaration._();
}

@freezed
sealed class AstStruct with _$AstStruct {
  const factory AstStruct({
    required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required AstStructKind kind,
  }) = _AstStruct;

  const AstStruct._();
}

@freezed
sealed class AstStructKind with _$AstStructKind {
  const factory AstStructKind.builtin() = AstStructKind_Builtin;
  const factory AstStructKind.userDefined({
    required List<AstStructField> fields,
  }) = AstStructKind_UserDefined;

  const AstStructKind._();
}

@freezed
sealed class AstStructField with _$AstStructField {
  const factory AstStructField({
    required AstErrorOr<Token> name,
    required AstErrorOr<AstType> type,
  }) = _AstStructField;

  const AstStructField._();
}

@freezed
sealed class AstEnum with _$AstEnum {
  const factory AstEnum({
    required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required List<AstEnumVariant> variants,
  }) = _AstEnum;

  const AstEnum._();
}

@freezed
sealed class AstEnumVariant with _$AstEnumVariant {
  const factory AstEnumVariant({
    required AstErrorOr<Token> name,
    required AstErrorOr<AstType>? type,
  }) = _AstEnumVariant;

  const AstEnumVariant._();
}

@freezed
sealed class AstTrait with _$AstTrait {
  const factory AstTrait({
    required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required List<AstFunction> functions,
  }) = _AstTrait;

  const AstTrait._();
}

@freezed
sealed class AstImpl with _$AstImpl {
  const factory AstImpl({
    required Token implKeyword,
    required AstTypeParameters? typeParameters,
    required AstErrorOr<AstType> type,
    required AstErrorOr<AstType> trait,
    required List<AstFunction> functions,
  }) = _AstImpl;

  const AstImpl._();
}

@freezed
sealed class AstAssignment with _$AstAssignment {
  const factory AstAssignment({
    required SourceSpan displaySpan,
    required AstErrorOr<Token> name,
    required AstErrorOr<AstType>? type,
    required AstErrorOr<AstExpression> value,
  }) = _AstAssignment;

  const AstAssignment._();
}

@freezed
sealed class AstFunction with _$AstFunction {
  const factory AstFunction({
    required SourceSpan displaySpan,
    required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required AstErrorOr<AstValueParameters> valueParameters,
    required AstType? returnType,
    required AstBody? body,
  }) = _AstFunction;

  const AstFunction._();
}

@freezed
sealed class AstValueParameters with _$AstValueParameters {
  const factory AstValueParameters({
    required List<AstValueParameter> valueParameters,
  }) = _AstValueParameters;

  const AstValueParameters._();
}

@freezed
sealed class AstValueParameter with _$AstValueParameter {
  const factory AstValueParameter({
    required AstErrorOr<Token> name,
    required AstErrorOr<AstType> type,
  }) = _AstValueParameter;

  const AstValueParameter._();
}

@freezed
sealed class AstTypeParameters with _$AstTypeParameters {
  const factory AstTypeParameters({
    required List<AstTypeParameter> parameters,
  }) = _AstTypeParameters;

  const AstTypeParameters._();
}

@freezed
sealed class AstTypeParameter with _$AstTypeParameter {
  const factory AstTypeParameter({
    required AstErrorOr<Token> name,
    required AstErrorOr<AstType>? upperBound,
  }) = _AstTypeParameter;

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
    required AstErrorOr<AstType> returnType,
  }) = AstType_Function;

  const AstType._();
}

@freezed
sealed class AstTypeArguments with _$AstTypeArguments {
  const factory AstTypeArguments({required List<AstType> arguments}) =
      _AstTypeArguments;

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
  // The parameter can't be named `call` since that would collide with the
  // generated `copyWith.call(…)`.
  const factory AstExpression.call(AstCall call_) = AstExpression_Call;
  const factory AstExpression.navigation(AstNavigation navigation) =
      AstExpression_Navigation;
  const factory AstExpression.lambda(AstLambda lambda) = AstExpression_Lambda;
  const factory AstExpression.body(AstBody body) = AstExpression_Body;
  const factory AstExpression.switch_(AstSwitch switch_) = AstExpression_Switch;

  const AstExpression._();
}

@freezed
sealed class AstTextPart with _$AstTextPart {
  const factory AstTextPart.text({
    required Token token,
    required String value,
  }) = AstTextPart_Text;
  const factory AstTextPart.interpolation({
    required AstErrorOr<AstExpression> expression,
  }) = AstTextPart_Interpolation;

  const AstTextPart._();
}

@freezed
sealed class AstParenthesized with _$AstParenthesized {
  const factory AstParenthesized({required AstErrorOr<AstExpression> inner}) =
      _AstParenthesized;

  const AstParenthesized._();
}

@freezed
sealed class AstCall with _$AstCall {
  const factory AstCall({
    required AstExpression receiver,
    required AstTypeArguments? typeArguments,
    required AstErrorOr<AstArguments> arguments,
  }) = _AstCall;

  const AstCall._();
}

@freezed
sealed class AstArguments with _$AstArguments {
  const factory AstArguments({
    required SourceSpan openingParenthesisSpan,
    required List<AstExpression> arguments,
  }) = _AstArguments;

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
    required AstErrorOr<Token> key,
  }) = _AstNavigation;

  const AstNavigation._();
}

@freezed
sealed class AstLambda with _$AstLambda {
  const factory AstLambda({
    required AstValueParameters parameters,
    required AstBody body,
  }) = _AstLambda;

  const AstLambda._();
}

@freezed
sealed class AstBody with _$AstBody {
  const factory AstBody({required List<AstStatement> statements}) = _AstBody;

  const AstBody._();
}

@freezed
sealed class AstStatement with _$AstStatement {
  const factory AstStatement.assignment(AstAssignment assignment) =
      AstStatement_Assignment;
  const factory AstStatement.expression(AstExpression expression) =
      AstStatement_Expression;

  const AstStatement._();
}

@freezed
sealed class AstSwitch with _$AstSwitch {
  const factory AstSwitch({
    required AstErrorOr<AstExpression> value,
    required List<AstSwitchCase> cases,
  }) = _AstSwitch;

  const AstSwitch._();
}

@freezed
sealed class AstSwitchCase with _$AstSwitchCase {
  const factory AstSwitchCase({
    required AstErrorOr<Token> variant,
    required AstErrorOr<Token>? valueName,
    required AstErrorOr<AstExpression> expression,
  }) = _AstSwitchCase;

  const AstSwitchCase._();
}
