import 'package:freezed_annotation/freezed_annotation.dart';

import 'expressions.dart';
import 'ids.dart';

part 'statements.freezed.dart';
part 'statements.g.dart';

@freezed
abstract class Statement implements _$Statement {
  const factory Statement.expression(
    DeclarationLocalId id,
    Expression expression,
  ) = ExpressionStatement;

  factory Statement.fromJson(Map<String, dynamic> json) =>
      _$StatementFromJson(json);
  const Statement._();
}
