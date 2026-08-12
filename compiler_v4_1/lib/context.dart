import 'dart:collection';

import 'error.dart';

class Context {
  final _errors = <CompilerError>[];
  List<CompilerError> get errors => UnmodifiableListView(_errors);

  void addError(CompilerError error) {
    _errors.add(error);
  }
}
