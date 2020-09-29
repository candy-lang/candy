import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:parser/parser.dart';

import 'compilation/ast.dart';

part 'errors.freezed.dart';

@freezed
abstract class CompilerError implements _$CompilerError {
  const factory CompilerError._create(String id) = _CompilerError;
  const CompilerError._();

  ReportedCompilerError call(String message, {ErrorLocation location}) =>
      ReportedCompilerError(this, message, location: location);

  static const values = [internalError, noMainFunction, multipleMainFunctions];
  static const internalError = CompilerError._create('internalError');
  static const noMainFunction = CompilerError._create('noMainFunction');
  static const multipleMainFunctions =
      CompilerError._create('multipleMainFunctions');
  static const multipleTypesWithSameName =
      CompilerError._create('multipleTypesWithSameName');
  static const undefinedIdentifier =
      CompilerError._create('undefinedIdentifier');
  static const unsupportedFeature = CompilerError._create('unsupportedFeature');
  static const moduleNotFound = CompilerError._create('moduleNotFound');
}

@freezed
abstract class ReportedCompilerError
    with _$ReportedCompilerError
    implements Exception {
  const factory ReportedCompilerError(
    CompilerError error,
    String message, {
    ErrorLocation location,
  }) = _ReportedCompilerError;
}

@freezed
abstract class ErrorLocation with _$ErrorLocation implements Exception {
  const factory ErrorLocation(ResourceId resourceId, [SourceSpan span]) =
      _ErrorLocation;
}
