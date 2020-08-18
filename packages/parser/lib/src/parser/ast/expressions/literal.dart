import 'package:freezed_annotation/freezed_annotation.dart';

import '../../../lexer/token.dart';
import '../../../source_span.dart';
import 'expression.dart';

part 'literal.freezed.dart';

@freezed
abstract class Literal<T> extends Expression implements _$Literal<T> {
  const factory Literal(LiteralToken<T> value) = _Literal;
  const Literal._();

  @override
  SourceSpan get span => value.span;
}
