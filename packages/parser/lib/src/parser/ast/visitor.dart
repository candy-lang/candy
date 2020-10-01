import '../../lexer/lexer.dart';

import 'declarations.dart';
import 'general.dart';

abstract class AstVisitor<R> {
  const AstVisitor();

  R visit(CandyFile file) {
    file.useLines.forEach(visitUseLine);
    visitDeclaration(file.declaration);
  }

  R visitUseLine(UseLine useLine) {
    visitKeywordToken(useLine.useKeyword);
    if (useLine.publisherName != null) {
      visitIdentifierToken(useLine.publisherName);
    }
    if (useLine.slash != null) visitOperatorToken(useLine.slash);
    visitIdentifierToken(useLine.packageName);
    if (useLine.dot != null) visitOperatorToken(useLine.dot);
    if (useLine.moduleName != null) visitIdentifierToken(useLine.moduleName);
  }

  R visitDeclaration(Declaration declaration) {}

  R visitOperatorToken(OperatorToken token) {}
  R visitKeywordToken(KeywordToken token) {}
  @deprecated
  R visitModifierToken(ModifierToken token) {}
  R visitLiteralToken(LiteralToken<dynamic> token) {}
  R visitIdentifierToken(IdentifierToken token) {}
}
