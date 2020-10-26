import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart' hide ValueParameter;

import '../body.dart';
import '../type.dart';
import 'declaration.dart';

final compileFunction = Query<DeclarationId, dart.Method>(
  'dart.compileFunction',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final functionHir = getFunctionDeclarationHir(context, declarationId);
    final isInsideTrait =
        declarationId.hasParent && declarationId.parent.isTrait;

    if (isInsideTrait && functionHir.isStatic) {
      throw CompilerError.unsupportedFeature(
        'Static functions in traits are not yet supported.',
        location: ErrorLocation(
          declarationId.resourceId,
          getPropertyDeclarationAst(context, declarationId)
              .modifiers
              .firstWhere((w) => w is StaticModifierToken)
              .span,
        ),
      );
    }

    return dart.Method((b) => b
      ..static = functionHir.isStatic && declarationId.parent.isNotModule
      ..returns = compileType(context, functionHir.returnType)
      ..name = functionHir.name
      ..types.addAll(functionHir.typeParameters
          .map((p) => compileTypeParameter(context, p)))
      ..requiredParameters
          .addAll(compileParameters(context, functionHir.valueParameters))
      ..body = compileBody(context, declarationId).valueOrNull);
  },
);

Iterable<dart.Parameter> compileParameters(
  QueryContext context,
  List<ValueParameter> parameters,
) {
  return parameters.map((p) => dart.Parameter((b) => b
    ..type = compileType(context, p.type)
    ..name = p.name));
}
