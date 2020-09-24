import 'dart:io';

import 'package:parser/src/parser/grammar.dart';

import 'src/parser/ast/general.dart';

export 'src/lexer/token.dart';
export 'src/parser/ast/declarations.dart';
export 'src/parser/ast/expressions/expressions.dart';
export 'src/parser/ast/expressions/operator.dart';
export 'src/parser/ast/general.dart';
export 'src/parser/ast/statements.dart';
export 'src/parser/ast/types.dart';

CandyFile parseCandyFile(File file) {
  assert(file != null);
  if (!file.existsSync()) {
    throw Exception("File ${file.absolute.path} doesn't exist.");
  }

  return parseCandySource(file.readAsStringSync());
}

CandyFile parseCandySource(String sourceCode) {
  final result = ParserGrammar.candyFile.parse(sourceCode);
  // TODO(JonasWanke): Better error handling for parser exceptions.
  return result.value;
}
