import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart';

import 'lexer/lexer.dart';
import 'parser/ast/declarations.dart';
import 'parser/ast/general.dart';
import 'parser/ast/node.dart';
import 'syntactic_entity.dart';

abstract class AstVisitor<R> {
  const AstVisitor();

  // lexer
  R visitOperatorToken(OperatorToken node);
  R visitKeywordToken(KeywordToken node);
  R visitModifierToken(ModifierToken node);
  R visitBoolLiteralToken(BoolLiteralToken node);
  R visitIntLiteralToken(IntLiteralToken node);
  R visitLiteralStringToken(LiteralStringToken node);
  R visitIdentifierToken(IdentifierToken node);

  // parser: general
  R visitCandyFile(CandyFile node);
  R visitUseLine(UseLine node);

  // parser: declarations
  R visitModuleDeclaration(ModuleDeclaration node);
  R visitTraitDeclaration(TraitDeclaration node);
  R visitImplDeclaration(ImplDeclaration node);
  R visitClassDeclaration(ClassDeclaration node);
  R visitBlockDeclarationBody(BlockDeclarationBody node);
  R visitConstructorCall(ConstructorCall node);
  R visitFunctionDeclaration(FunctionDeclaration node);
  R visitValueParameter(ValueParameter node);
  R visitPropertyDeclaration(PropertyDeclaration node);
  R visitPropertyAccessor(PropertyAccessor node);

  // parser: types
  R visitUserType(UserType node);
  R visitSimpleUserType(SimpleUserType node);
  R visitGroupType(GroupType node);
  R visitFunctionType(FunctionType node);
  R visitTupleType(TupleType node);
  R visitUnionType(UnionType node);
  R visitIntersectionType(IntersectionType node);
  R visitTypeParameters(TypeParameters node);
  R visitTypeParameter(TypeParameter node);
  R visitTypeArguments(TypeArguments node);
  R visitTypeArgument(TypeArgument node);

  // parser: expressions
  R visitLiteral(Literal<dynamic> node);
  R visitStringLiteral(StringLiteral node);
  R visitStringLiteralPart(StringLiteralPart node);
  R visitLambdaLiteral(LambdaLiteral node);
  R visitIdentifier(Identifier node);
  R visitGroupExpression(GroupExpression node);
  R visitPrefixExpression(PrefixExpression node);
  R visitPostfixExpression(PostfixExpression node);
  R visitBinaryExpression(BinaryExpression node);
  R visitNavigationExpression(NavigationExpression node);
  R visitCallExpression(CallExpression node);
  R visitArgument(Argument node);
  R visitIndexExpression(IndexExpression node);
  R visitIsExpression(IsExpression node);
  R visitIfExpression(IfExpression node);
  R visitLoopExpression(LoopExpression node);
  R visitWhileExpression(WhileExpression node);
  R visitReturnExpression(ReturnExpression node);
  R visitBreakExpression(BreakExpression node);
  R visitContinueExpression(ContinueExpression node);
  R visitThrowExpression(ThrowExpression node);
  R visitPropertyDeclarationExpression(PropertyDeclarationExpression node);
}

abstract class GeneralizingAstVisitor<R> extends AstVisitor<R> {
  const GeneralizingAstVisitor();

  R visitSyntacticEntity(SyntacticEntity node);

  // lexer
  R visitToken(Token node) => visitSyntacticEntity(node);
  @override
  R visitOperatorToken(OperatorToken node) => visitToken(node);
  @override
  R visitKeywordToken(KeywordToken node) => visitToken(node);
  @override
  R visitModifierToken(ModifierToken node) => visitToken(node);
  R visitLiteralToken(LiteralToken<dynamic> node) => visitToken(node);
  @override
  R visitBoolLiteralToken(BoolLiteralToken node) => visitLiteralToken(node);
  @override
  R visitIntLiteralToken(IntLiteralToken node) => visitLiteralToken(node);
  @override
  R visitLiteralStringToken(LiteralStringToken node) => visitToken(node);
  @override
  R visitIdentifierToken(IdentifierToken node) => visitToken(node);

  // parser
  R visitAstNode(AstNode node) => visitSyntacticEntity(node);
  // parser: general
  @override
  R visitCandyFile(CandyFile node) => visitAstNode(node);
  @override
  R visitUseLine(UseLine node) => visitAstNode(node);

  // parser: declarations
  R visitDeclaration(Declaration node) => visitAstNode(node);
  @override
  R visitModuleDeclaration(ModuleDeclaration node) => visitDeclaration(node);
  @override
  R visitTraitDeclaration(TraitDeclaration node) => visitDeclaration(node);
  @override
  R visitImplDeclaration(ImplDeclaration node) => visitDeclaration(node);
  @override
  R visitClassDeclaration(ClassDeclaration node) => visitDeclaration(node);
  @override
  R visitBlockDeclarationBody(BlockDeclarationBody node) => visitAstNode(node);
  @override
  R visitConstructorCall(ConstructorCall node) => visitAstNode(node);
  @override
  R visitFunctionDeclaration(FunctionDeclaration node) =>
      visitDeclaration(node);
  @override
  R visitValueParameter(ValueParameter node) => visitAstNode(node);
  @override
  R visitPropertyDeclaration(PropertyDeclaration node) =>
      visitDeclaration(node);
  @override
  R visitPropertyAccessor(PropertyAccessor node) => visitDeclaration(node);

  // parser: types
  R visitType(Type node) => visitAstNode(node);
  @override
  R visitUserType(UserType node) => visitType(node);
  @override
  R visitSimpleUserType(SimpleUserType node) => visitType(node);
  @override
  R visitGroupType(GroupType node) => visitType(node);
  @override
  R visitFunctionType(FunctionType node) => visitType(node);
  @override
  R visitTupleType(TupleType node) => visitType(node);
  @override
  R visitUnionType(UnionType node) => visitType(node);
  @override
  R visitIntersectionType(IntersectionType node) => visitType(node);
  @override
  R visitTypeParameters(TypeParameters node) => visitAstNode(node);
  @override
  R visitTypeParameter(TypeParameter node) => visitAstNode(node);
  @override
  R visitTypeArguments(TypeArguments node) => visitAstNode(node);
  @override
  R visitTypeArgument(TypeArgument node) => visitAstNode(node);

  // parser: expressions
  R visitExpression(Expression node) => visitAstNode(node);
  @override
  R visitLiteral(Literal<dynamic> node) => visitExpression(node);
  @override
  R visitStringLiteral(StringLiteral node) => visitExpression(node);
  @override
  R visitStringLiteralPart(StringLiteralPart node) => visitAstNode(node);
  @override
  R visitLambdaLiteral(LambdaLiteral node) => visitExpression(node);
  @override
  R visitIdentifier(Identifier node) => visitExpression(node);
  @override
  R visitGroupExpression(GroupExpression node) => visitExpression(node);
  R visitOperatorExpression(OperatorExpression node) => visitExpression(node);
  R visitUnaryExpression(UnaryExpression node) => visitOperatorExpression(node);
  @override
  R visitPrefixExpression(PrefixExpression node) => visitUnaryExpression(node);
  @override
  R visitPostfixExpression(PostfixExpression node) =>
      visitUnaryExpression(node);
  @override
  R visitBinaryExpression(BinaryExpression node) =>
      visitOperatorExpression(node);
  @override
  R visitNavigationExpression(NavigationExpression node) =>
      visitExpression(node);
  @override
  R visitCallExpression(CallExpression node) => visitExpression(node);
  @override
  R visitArgument(Argument node) => visitAstNode(node);
  @override
  R visitIndexExpression(IndexExpression node) => visitExpression(node);
  @override
  R visitIsExpression(IsExpression node) => visitExpression(node);
  @override
  R visitIfExpression(IfExpression node) => visitExpression(node);
  @override
  R visitLoopExpression(LoopExpression node) => visitExpression(node);
  @override
  R visitWhileExpression(WhileExpression node) => visitExpression(node);
  @override
  R visitReturnExpression(ReturnExpression node) => visitExpression(node);
  @override
  R visitBreakExpression(BreakExpression node) => visitExpression(node);
  @override
  R visitContinueExpression(ContinueExpression node) => visitExpression(node);
  @override
  R visitThrowExpression(ThrowExpression node) => visitExpression(node);
  @override
  R visitPropertyDeclarationExpression(PropertyDeclarationExpression node) =>
      visitExpression(node);
}

class NodeFinderVisitor extends GeneralizingAstVisitor<AstNode> {
  const NodeFinderVisitor._(this.offset) : assert(offset != null);

  static AstNode find(CandyFile file, int offset) =>
      file.accept(NodeFinderVisitor._(offset));

  final int offset;

  @override
  AstNode visitSyntacticEntity(SyntacticEntity node) {
    throw StateError('This should never be called.');
  }

  @override
  AstNode visitToken(Token node) {
    throw StateError('This should never be called.');
  }

  @override
  AstNode visitAstNode(AstNode node) {
    assert(node.span.contains(offset));

    final childMatches = node.children
        .whereType<AstNode>()
        .where((it) => it.span.contains(offset));
    assert(childMatches.length < 2);
    return childMatches.firstOrNull?.accept(this) ?? node;
  }
}

abstract class TraversingAstVisitor extends GeneralizingAstVisitor<void> {
  const TraversingAstVisitor();

  @override
  void visitSyntacticEntity(SyntacticEntity node) {}

  @override
  void visitAstNode(AstNode node) {
    for (final innerNode in node.children) innerNode.accept(this);
  }
}

class ExpressionFinderVisitor extends TraversingAstVisitor {
  ExpressionFinderVisitor._(this.id) : assert(id != null);

  static AstNode find(CandyFile file, int id) {
    final visitor = ExpressionFinderVisitor._(id);
    file.accept(visitor);
    return visitor._result;
  }

  final int id;
  Expression _result;

  @override
  void visitExpression(Expression node) {
    if (node.id == id) {
      _result = node;
      return;
    }
    super.visitExpression(node);
  }
}
