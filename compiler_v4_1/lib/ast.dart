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
sealed class const Ast._() with _$Ast {
  const factory({
    required List<AstUse> uses,
    required List<AstDeclaration> declarations,
  }) = _Ast;

  factory fromJson(Map<String, dynamic> json) => _$AstFromJson(json);
}

@freezed
sealed class const AstUse._() with _$AstUse {
  const factory({required Token useKeyword, required List<Token> path}) =
      _AstUse;

  factory fromJson(Map<String, dynamic> json) => _$AstUseFromJson(json);
}

// Declarations

@freezed
sealed class const AstDeclaration._() with _$AstDeclaration {
  const factory struct(AstStruct struct) = AstDeclaration_Struct;
  const factory enum_(AstEnum enum_) = AstDeclaration_Enum;
  const factory trait(AstTrait trait) = AstDeclaration_Trait;
  const factory impl(AstImpl impl) = AstDeclaration_Impl;
  const factory assignment(AstAssignment assignment) =
      AstDeclaration_Assignment;
  const factory function(AstFunction function) = AstDeclaration_Function;

  factory fromJson(Map<String, dynamic> json) => _$AstDeclarationFromJson(json);
}

@freezed
sealed class const AstStruct._() with _$AstStruct {
  const factory({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required AstStructKind kind,
  }) = _AstStruct;

  factory fromJson(Map<String, dynamic> json) => _$AstStructFromJson(json);
}

@freezed
sealed class const AstStructKind._() with _$AstStructKind {
  const factory builtin() = AstStructKind_Builtin;
  const factory userDefined({required List<AstStructField> fields}) =
      AstStructKind_UserDefined;

  factory fromJson(Map<String, dynamic> json) => _$AstStructKindFromJson(json);
}

@freezed
sealed class const AstStructField._() with _$AstStructField {
  const factory({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required AstErrorOr<AstType> type,
  }) = _AstStructField;

  factory fromJson(Map<String, dynamic> json) => _$AstStructFieldFromJson(json);
}

@freezed
sealed class const AstEnum._() with _$AstEnum {
  const factory({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required List<AstEnumVariant> variants,
  }) = _AstEnum;

  factory fromJson(Map<String, dynamic> json) => _$AstEnumFromJson(json);
}

@freezed
sealed class const AstEnumVariant._() with _$AstEnumVariant {
  const factory({
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? type,
  }) = _AstEnumVariant;

  factory fromJson(Map<String, dynamic> json) => _$AstEnumVariantFromJson(json);
}

@freezed
sealed class const AstTrait._() with _$AstTrait {
  const factory({
    @Default(AstVisibility.private) AstVisibility visibility,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    required List<AstAssociatedType> associatedTypes,
    required List<AstFunction> functions,
  }) = _AstTrait;

  factory fromJson(Map<String, dynamic> json) => _$AstTraitFromJson(json);
}

@freezed
sealed class const AstImpl._() with _$AstImpl {
  const factory({
    required Token implKeyword,
    required AstTypeParameters? typeParameters,
    @_AstTypeOptionConverter() required AstErrorOr<AstType> type,
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? trait,
    required List<AstAssociatedType> associatedTypes,
    required List<AstFunction> functions,
  }) = _AstImpl;

  factory fromJson(Map<String, dynamic> json) => _$AstImplFromJson(json);
}

/// `type Item: Bound` in a trait, `type Item = Int` in an impl. Both forms
/// parse everywhere; signature lowering rejects the one that doesn't belong.
@freezed
sealed class const AstAssociatedType._() with _$AstAssociatedType {
  const factory({
    required SourceSpan displaySpan,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required List<AstErrorOr<AstType>> upperBounds,
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? value,
  }) = _AstAssociatedType;

  factory fromJson(Map<String, dynamic> json) =>
      _$AstAssociatedTypeFromJson(json);
}

@freezed
sealed class const AstAssignment._() with _$AstAssignment {
  const factory({
    @Default(AstVisibility.private) AstVisibility visibility,
    required SourceSpan displaySpan,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? type,
    @_AstExpressionOptionConverter() required AstErrorOr<AstExpression> value,
  }) = _AstAssignment;

  factory fromJson(Map<String, dynamic> json) => _$AstAssignmentFromJson(json);
}

@freezed
sealed class const AstFunction._() with _$AstFunction {
  const factory({
    @Default(AstVisibility.private) AstVisibility visibility,
    required SourceSpan displaySpan,
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    required AstTypeParameters? typeParameters,
    @_AstValueParametersOptionConverter()
    required AstErrorOr<AstValueParameters> valueParameters,
    required AstType? returnType,
    required AstBody? body,
  }) = _AstFunction;

  factory fromJson(Map<String, dynamic> json) => _$AstFunctionFromJson(json);
}

@freezed
sealed class const AstValueParameters._()
    with _$AstValueParameters
    implements ToJsonMap {
  const factory({required List<AstValueParameter> valueParameters}) =
      _AstValueParameters;

  factory fromJson(Map<String, dynamic> json) =>
      _$AstValueParametersFromJson(json);
}

@freezed
sealed class const AstValueParameter._() with _$AstValueParameter {
  const factory({
    @_TokenOptionConverter() required AstErrorOr<Token> name,

    /// `null` when the parameter has no annotation, which only lambdas allow.
    @_AstTypeOptionConverter() required AstErrorOr<AstType>? type,
  }) = _AstValueParameter;

  factory fromJson(Map<String, dynamic> json) =>
      _$AstValueParameterFromJson(json);
}

@freezed
sealed class const AstTypeParameters._() with _$AstTypeParameters {
  const factory({required List<AstTypeParameter> parameters}) =
      _AstTypeParameters;

  factory fromJson(Map<String, dynamic> json) =>
      _$AstTypeParametersFromJson(json);
}

@freezed
sealed class const AstTypeParameter._() with _$AstTypeParameter {
  const factory({
    @_TokenOptionConverter() required AstErrorOr<Token> name,
    @_AstTypeOptionConverter() required List<AstErrorOr<AstType>> upperBounds,
  }) = _AstTypeParameter;

  factory fromJson(Map<String, dynamic> json) =>
      _$AstTypeParameterFromJson(json);
}

// Types

@freezed
sealed class const AstType._() with _$AstType implements ToJsonMap {
  const factory named({
    required Token name,
    required AstTypeArguments? typeArguments,
  }) = AstType_Named;
  const factory function({
    required List<AstType> parameterTypes,
    @_AstTypeOptionConverter() required AstErrorOr<AstType> returnType,
  }) = AstType_Function;

  /// `T.Item`: the associated type [name] of whichever trait [baseType] gets it
  /// from.
  const factory associated({required AstType baseType, required Token name}) =
      AstType_Associated;

  factory fromJson(Map<String, dynamic> json) => _$AstTypeFromJson(json);
}

@freezed
sealed class const AstTypeArguments._() with _$AstTypeArguments {
  const factory({required List<AstType> arguments}) = _AstTypeArguments;

  factory fromJson(Map<String, dynamic> json) =>
      _$AstTypeArgumentsFromJson(json);
}

// Expressions

@freezed
sealed class const AstExpression._() with _$AstExpression implements ToJsonMap {
  const factory int({required int value, required Token token}) =
      AstExpression_Int;
  const factory identifier(Token identifier) = AstExpression_Identifier;
  const factory text({required List<AstTextPart> parts}) = AstExpression_Text;
  const factory parenthesized(AstParenthesized parenthesized) =
      AstExpression_Parenthesized;
  // The parameter can't be named `call` since that would collide with the
  // generated `copyWith.call(…)`.
  const factory call(AstCall call_) = AstExpression_Call;
  const factory navigation(AstNavigation navigation) = AstExpression_Navigation;
  const factory lambda(AstLambda lambda) = AstExpression_Lambda;
  const factory body(AstBody body) = AstExpression_Body;
  const factory switch_(AstSwitch switch_) = AstExpression_Switch;

  factory fromJson(Map<String, dynamic> json) => _$AstExpressionFromJson(json);
}

@freezed
sealed class const AstTextPart._() with _$AstTextPart {
  const factory text({required Token token, required String value}) =
      AstTextPart_Text;
  const factory interpolation({
    @_AstExpressionOptionConverter()
    required AstErrorOr<AstExpression> expression,
  }) = AstTextPart_Interpolation;

  factory fromJson(Map<String, dynamic> json) => _$AstTextPartFromJson(json);
}

@freezed
sealed class const AstParenthesized._() with _$AstParenthesized {
  const factory({
    @_AstExpressionOptionConverter() required AstErrorOr<AstExpression> inner,
  }) = _AstParenthesized;

  factory fromJson(Map<String, dynamic> json) =>
      _$AstParenthesizedFromJson(json);
}

@freezed
sealed class const AstCall._() with _$AstCall {
  const factory({
    required AstExpression receiver,
    required AstTypeArguments? typeArguments,
    @_AstArgumentsOptionConverter() required AstErrorOr<AstArguments> arguments,
  }) = _AstCall;

  factory fromJson(Map<String, dynamic> json) => _$AstCallFromJson(json);
}

@freezed
sealed class const AstArguments._() with _$AstArguments implements ToJsonMap {
  const factory({
    required SourceSpan openingParenthesisSpan,
    required List<AstExpression> arguments,
  }) = _AstArguments;

  factory fromJson(Map<String, dynamic> json) => _$AstArgumentsFromJson(json);
}

extension AstErrorOrAstArguments on AstErrorOr<AstArguments> {
  List<AstExpression> get argumentsOrDefault =>
      toNullable()?.arguments ?? const [];
}

@freezed
sealed class const AstNavigation._() with _$AstNavigation {
  const factory({
    required AstExpression receiver,
    @_TokenOptionConverter() required AstErrorOr<Token> key,
  }) = _AstNavigation;

  factory fromJson(Map<String, dynamic> json) => _$AstNavigationFromJson(json);
}

@freezed
sealed class const AstLambda._() with _$AstLambda {
  const factory({
    required AstValueParameters parameters,
    required AstBody body,
  }) = _AstLambda;

  factory fromJson(Map<String, dynamic> json) => _$AstLambdaFromJson(json);
}

@freezed
sealed class const AstBody._() with _$AstBody {
  const factory({required List<AstStatement> statements}) = _AstBody;

  factory fromJson(Map<String, dynamic> json) => _$AstBodyFromJson(json);
}

@freezed
sealed class const AstStatement._() with _$AstStatement {
  const factory assignment(AstAssignment assignment) = AstStatement_Assignment;
  const factory expression(AstExpression expression) = AstStatement_Expression;

  factory fromJson(Map<String, dynamic> json) => _$AstStatementFromJson(json);
}

@freezed
sealed class const AstSwitch._() with _$AstSwitch {
  const factory({
    @_AstExpressionOptionConverter() required AstErrorOr<AstExpression> value,
    required List<AstSwitchCase> cases,
  }) = _AstSwitch;

  factory fromJson(Map<String, dynamic> json) => _$AstSwitchFromJson(json);
}

@freezed
sealed class const AstSwitchCase._() with _$AstSwitchCase {
  const factory({
    @_TokenOptionConverter() required AstErrorOr<Token> variant,
    @_TokenOptionConverter() required AstErrorOr<Token>? valueName,
    @_AstExpressionOptionConverter()
    required AstErrorOr<AstExpression> expression,
  }) = _AstSwitchCase;

  factory fromJson(Map<String, dynamic> json) => _$AstSwitchCaseFromJson(json);
}

class _TokenOptionConverter
    extends OptionConverter<Token, Map<String, dynamic>> {
  const new() : super(Token.fromJson);
}

class _AstTypeOptionConverter
    extends OptionConverter<AstType, Map<String, dynamic>> {
  const new() : super(AstType.fromJson);
}

class _AstExpressionOptionConverter
    extends OptionConverter<AstExpression, Map<String, dynamic>> {
  const new() : super(AstExpression.fromJson);
}

class _AstArgumentsOptionConverter
    extends OptionConverter<AstArguments, Map<String, dynamic>> {
  const new() : super(AstArguments.fromJson);
}

class _AstValueParametersOptionConverter
    extends OptionConverter<AstValueParameters, Map<String, dynamic>> {
  const new() : super(AstValueParameters.fromJson);
}
