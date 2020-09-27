import 'dart:io';

import 'package:path/path.dart' as p;

import 'src/parser/ast/general.dart';
import 'src/parser/grammar.dart';

export 'src/lexer/token.dart';
export 'src/parser/ast/declarations.dart';
export 'src/parser/ast/expressions/expressions.dart';
export 'src/parser/ast/expressions/operator.dart';
export 'src/parser/ast/general.dart';
export 'src/parser/ast/statements.dart';
export 'src/parser/ast/types.dart';
export 'src/source_span.dart';

CandyFile parseCandyFile(File file) {
  assert(file != null);
  if (!file.existsSync()) {
    throw Exception("File ${file.absolute.path} doesn't exist.");
  }

  return parseCandySource(
    p.basenameWithoutExtension(file.path),
    file.readAsStringSync(),
  );
}

CandyFile parseCandySource(String fileNameWithoutExtension, String sourceCode) {
  // TODO(JonasWanke): Better error handling for parser exceptions.
  return ParserGrammar.parse(fileNameWithoutExtension, sourceCode);
}
