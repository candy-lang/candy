import 'package:meta/meta.dart';

import 'lexer/token.dart';
import 'parser/ast/node.dart';
import 'source_span.dart';

/// Base class for [Token] and [AstNode].
@immutable
abstract class SyntacticEntity {
  const SyntacticEntity();

  /// The [SourceSpan] of this node in the source file or `null` if this node is
  /// synthetic.
  SourceSpan get span;
}
