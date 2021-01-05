import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';
import 'declaration.dart';

final compileFunction = Query<DeclarationId, dart.Method>(
  'dart.compileFunction',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final functionHir = getFunctionDeclarationHir(context, declarationId);

    final body = compileBody(context, declarationId).valueOrNull;

    return dart.Method((b) => b
      ..static = functionHir.isStatic && declarationId.parent.isNotModule
      ..returns = compileType(context, functionHir.returnType)
      ..name = functionHir.name
      ..types.addAll(functionHir.typeParameters
          .map((p) => compileTypeParameter(context, p)))
      ..requiredParameters
          .addAll(compileParameters(context, functionHir.valueParameters))
      ..body = body);
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
