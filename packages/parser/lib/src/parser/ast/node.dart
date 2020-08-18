import 'package:meta/meta.dart';

import '../../source_span.dart';
import '../../syntactic_entity.dart';

@immutable
abstract class AstNode extends SyntacticEntity {
  const AstNode();

  @override
  SourceSpan get span {
    final childs = children;
    assert(childs.isNotEmpty);
    return SourceSpan(childs.first.span.start, childs.last.span.end);
  }

  Iterable<SyntacticEntity> get children => [];
}
