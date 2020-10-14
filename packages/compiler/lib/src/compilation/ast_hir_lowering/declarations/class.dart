import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';

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
final getClassDeclarationHir = Query<DeclarationId, hir.ClassDeclaration>(
  'getClassDeclarationHir',
  provider: (context, declarationId) {
    final ast = context.callQuery(getClassDeclarationAst, declarationId);
    final moduleId = context.callQuery(declarationIdToModuleId, declarationId);
    return hir.ClassDeclaration(
      name: ast.name.name,
      typeParameters: ast.typeParameters?.parameters.orEmpty
          .map((p) => hir.TypeParameter(
                name: p.name.name,
                upperBound:
                    astTypeToHirType(context, Tuple2(moduleId, p.bound)),
              ))
          .toList(),
      innerDeclarationIds: getInnerDeclarationIds(
        context,
        moduleIdToDeclarationId(context, moduleId),
      ),
    );
  },
);
