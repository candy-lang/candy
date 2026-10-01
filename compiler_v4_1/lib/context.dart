import 'package:supernova/supernova.dart';

import 'error.dart';
import 'position.dart';
import 'references.dart';

class Context {
  final _errors = <CompilerError>{};
  List<CompilerError> get errors => _errors.sorted();

  final references = ReferenceIndex();

  void addError(CompilerError error) {
    _errors.add(error);
  }

  /// Names a place something is declared, whether or not anything uses it.
  void addDeclaration(SourceSpan span) => references.addDeclaration(span);

  /// [targets] may be empty, which records nothing: a name that didn't resolve
  /// simply has nowhere to jump to.
  void addReference(SourceSpan span, Iterable<SourceSpan?> targets) =>
      references.add(span, targets.nonNulls);
}
