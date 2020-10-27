import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:parser/parser.dart';

import 'compilation/ast.dart';

part 'errors.freezed.dart';
part 'errors.g.dart';

@freezed
abstract class CompilerError implements _$CompilerError {
  const factory CompilerError._create(String id) = _CompilerError;
  factory CompilerError.fromJson(Map<String, dynamic> json) =>
      _$CompilerErrorFromJson(json);
  const CompilerError._();

  ReportedCompilerError call(
    String message, {
    ErrorLocation location,
    List<ErrorRelatedInformation> relatedInformation = const [],
  }) {
    return ReportedCompilerError(
      this,
      message,
      location: location,
      relatedInformation: relatedInformation,
    );
  }

  static const values = [internalError, noMainFunction, multipleMainFunctions];
  static const ambiguousExpression =
      CompilerError._create('ambiguousExpression');
  static const ambiguousImplsFound =
      CompilerError._create('ambiguousImplsFound');
  static const assignmentToImmutable =
      CompilerError._create('assignmentToImmutable');
  static const candyspecMissing = CompilerError._create('candyspecMissing');
  static const duplicateArgument = CompilerError._create('duplicateArgument');
  static const internalError = CompilerError._create('internalError');
  static const invalidArgumentType =
      CompilerError._create('invalidArgumentType');
  static const invalidExpressionType =
      CompilerError._create('invalidExpressionType');
  static const invalidImplTraitBound =
      CompilerError._create('invalidImplTraitBound');
  static const invalidLabel = CompilerError._create('invalidLabel');
  static const invalidUseLine = CompilerError._create('invalidUseLine');
  static const lambdaParameterTypeRequired =
      CompilerError._create('lambdaParameterTypeRequired');
  static const lambdaParametersMissing =
      CompilerError._create('lambdaParametersMissing');
  static const missingReturn = CompilerError._create('missingReturn');
  static const moduleNotFound = CompilerError._create('moduleNotFound');
  static const multipleMainFunctions =
      CompilerError._create('multipleMainFunctions');
  static const multipleTypesWithSameName =
      CompilerError._create('multipleTypesWithSameName');
  static const noMainFunction = CompilerError._create('noMainFunction');
  static const propertyInitializerMissing =
      CompilerError._create('propertyInitializerMissing');
  static const propertyTypeOrValueRequired =
      CompilerError._create('propertyTypeOrValueRequired');
  static const tooManyArguments = CompilerError._create('tooManyArguments');
  static const typeNotFound = CompilerError._create('typeNotFound');
  static const returnNotInFunction =
      CompilerError._create('returnNotInFunction');
  static const undefinedIdentifier =
      CompilerError._create('undefinedIdentifier');
  static const unexpectedPositionalArgument =
      CompilerError._create('unexpectedPositionalArgument');
  static const unknownPackage = CompilerError._create('unknownPackage');
  static const unsupportedFeature = CompilerError._create('unsupportedFeature');
}

@freezed
abstract class ReportedCompilerError
    with _$ReportedCompilerError
    implements Exception {
  const factory ReportedCompilerError(
    CompilerError error,
    String message, {
    ErrorLocation location,
    @Default(<ErrorRelatedInformation>[])
        List<ErrorRelatedInformation> relatedInformation,
  }) = _ReportedCompilerError;
  factory ReportedCompilerError.fromJson(Map<String, dynamic> json) =>
      _$ReportedCompilerErrorFromJson(json);
}

@freezed
abstract class ErrorRelatedInformation
    with _$ErrorRelatedInformation
    implements Exception {
  const factory ErrorRelatedInformation({
    @required ErrorLocation location,
    @required String message,
  }) = _ErrorRelatedInformation;
  factory ErrorRelatedInformation.fromJson(Map<String, dynamic> json) =>
      _$ErrorRelatedInformationFromJson(json);
}

@freezed
abstract class ErrorLocation with _$ErrorLocation implements Exception {
  const factory ErrorLocation(ResourceId resourceId, [SourceSpan span]) =
      _ErrorLocation;
  factory ErrorLocation.fromJson(Map<String, dynamic> json) =>
      _$ErrorLocationFromJson(json);
}
