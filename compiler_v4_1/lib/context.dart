import 'error.dart';
import 'position.dart';
import 'references.dart';

class Context {
  final _errors = <CompilerError>{};
  List<CompilerError> get errors => _errors.toList()..sort();

  void addError(CompilerError error) {
    _errors.add(error);
  }
}
