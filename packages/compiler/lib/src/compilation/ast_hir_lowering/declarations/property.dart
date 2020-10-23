import 'package:compiler/src/errors.dart';
import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../body.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';

extension PropertyDeclarationId on DeclarationId {
  bool get isProperty =>
      path.isNotEmpty && path.last.data is PropertyDeclarationPathData;
  bool get isNotProperty => !isProperty;
}

final getPropertyDeclarationAst = Query<DeclarationId, ast.PropertyDeclaration>(
  'getPropertyDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isProperty);

    final declaration = getDeclarationAst(context, declarationId);
    assert(declaration is ast.PropertyDeclaration, 'Wrong return type.');
    return declaration as ast.PropertyDeclaration;
  },
);
final getPropertyDeclarationHir = Query<DeclarationId, hir.PropertyDeclaration>(
  'getPropertyDeclarationHir',
  provider: (context, declarationId) {
    final propertyAst = getPropertyDeclarationAst(context, declarationId);

    if (propertyAst.type == null && propertyAst.initializer == null) {
      throw CompilerError.propertyTypeOrValueRequired(
        'Property `${propertyAst.name.name}` is declared without an explicit type or a default value.',
        location:
            ErrorLocation(declarationId.resourceId, propertyAst.name.span),
      );
    }

    hir.Expression initializer;
    if (propertyAst.initializer != null) {
      final result = getBody(context, declarationId).value;
      assert(result.length == 1);
      initializer = result.single;
    }

    return hir.PropertyDeclaration(
      isStatic: propertyAst.isStatic,
      isMutable: propertyAst.isMutable,
      name: propertyAst.name.name,
      type: propertyAst.type != null
          ? astTypeToHirType(context, Tuple2(declarationId, propertyAst.type))
          : initializer.type,
      initializer: initializer,
      innerDeclarationIds: getInnerDeclarationIds(context, declarationId),
    );
  },
);
