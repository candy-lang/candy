import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:petitparser/petitparser.dart';

void main() {
  ParserGrammar.init();
  print(ParserGrammar.expression);
  print(LexerGrammar.RETURN.parse('return'));
  final dynamic result = ParserGrammar.expression.parse('return 123');
  print(result);
}
