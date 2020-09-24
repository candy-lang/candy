import 'package:parser/parser.dart' as ast;

import '../../query.dart';
import '../hir/ids.dart';
import 'declaration.dart';

extension ModuleDeclarationId on DeclarationId {
  bool get isModule =>
      path.isEmpty || path.last.data is ModuleDeclarationPathData;
}

final getModuleDeclarationAst = Query<DeclarationId, ast.ModuleDeclaration>(
  'getModuleDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isModule);

    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration != null, 'Module $declarationId not found.');
    assert(declaration is ast.ModuleDeclaration, 'Wrong return type.');
    return declaration as ast.ModuleDeclaration;
  },
);
