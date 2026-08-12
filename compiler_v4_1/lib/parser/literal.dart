import 'parser.dart';

extension LiteralParser on Parser {
  Parser? equalsSign(Parser parser) => parser.consumeLiteral('=');
  Parser? comma(Parser parser) => parser.consumeLiteral(',');
  Parser? dot(Parser parser) => parser.consumeLiteral('.');
  Parser? colon(Parser parser) => parser.consumeLiteral(':');
  Parser? openingParenthesis(Parser parser) => parser.consumeLiteral('(');
  Parser? closingParenthesis(Parser parser) => parser.consumeLiteral(')');
  Parser? openingBracket(Parser parser) => parser.consumeLiteral('[');
  Parser? closingBracket(Parser parser) => parser.consumeLiteral(']');
  Parser? openingCurlyBrace(Parser parser) => parser.consumeLiteral('{');
  Parser? closingCurlyBrace(Parser parser) => parser.consumeLiteral('}');
  Parser? doubleQuote(Parser parser) => parser.consumeLiteral('"');
  Parser? octothorpe(Parser parser) => parser.consumeLiteral('#');
  Parser? arrow(Parser parser) => parser.consumeLiteral('=>');


  Parser? builtinKeyword(Parser parser) => parser.consumeLiteral('builtin');
  Parser? structKeyword(Parser parser) => parser.consumeLiteral('struct');
  Parser? enumKeyword(Parser parser) => parser.consumeLiteral('enum');
  Parser? traitKeyword(Parser parser) => parser.consumeLiteral('trait');
  Parser? implKeyword(Parser parser) => parser.consumeLiteral('impl');
  Parser? funKeyword(Parser parser) => parser.consumeLiteral('fun');
  Parser? letKeyword(Parser parser) => parser.consumeLiteral('let');
  Parser? switchKeyword(Parser parser) => parser.consumeLiteral('switch');

  bool consumeKeyword(Keyword keyword) {

  }
}
