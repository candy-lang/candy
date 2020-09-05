import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/lexer.dart';
import '../../syntactic_entity.dart';
import 'node.dart';

part 'general.freezed.dart';

@freezed
abstract class UseLine extends AstNode implements _$UseLine {
  const factory UseLine({
    @required UseKeywordToken useKeyword,
    IdentifierToken publisherName,
    OperatorToken slash,
    @required IdentifierToken packageName,
    OperatorToken dot,
    IdentifierToken moduleName,
  }) = _UseLine;
  const UseLine._();

  @override
  Iterable<SyntacticEntity> get children => [
        useKeyword,
        if (publisherName != null) publisherName,
        if (slash != null) slash,
        packageName,
        if (dot != null) dot,
        if (moduleName != null) moduleName,
      ];
}
