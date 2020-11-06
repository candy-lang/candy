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

    final start = childs.firstWhere((it) => it.span != null)?.span?.start;
    final end = childs.lastWhere((it) => it.span != null)?.span?.end;
    assert((start != null) == (end != null));
    if (start == null) return null;

    return SourceSpan(start, end);
  }

  Iterable<SyntacticEntity> get children => [];
}
