import 'package:supernova/supernova.dart';

import 'position.dart';

part 'error.freezed.dart';

@freezed
sealed class CompilerError with _$CompilerError {
  const factory CompilerError({
    required SourceSpan span,
    required String message,
  }) = _CompilerError;
  const CompilerError._();
}
