import 'error.dart';

class Context {
  final _errors = <CompilerError>[];
  void addError(CompilerError error) {
    _errors.add(error);
  }
}
