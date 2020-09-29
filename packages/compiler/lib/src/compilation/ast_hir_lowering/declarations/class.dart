import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../hir/ids.dart';
import 'declarations.dart';

extension ClassDeclarationId on DeclarationId {
  bool get isClass => path.last.data is ClassDeclarationPathData;
  bool get isNotClass => !isClass;
}

final getClassDeclarationAst = Query<DeclarationId, ast.ClassDeclaration>(
  'getClassDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isClass);

    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration is ast.ClassDeclaration, 'Wrong return type.');
    return declaration as ast.ClassDeclaration;
  },
);
