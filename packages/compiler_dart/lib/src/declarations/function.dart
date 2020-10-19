import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart' hide ValueParameter;

import '../body.dart';
import '../constants.dart';
import '../type.dart';

final compileFunction = Query<DeclarationId, dart.Method>(
  'dart.compileFunction',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final function = getFunctionDeclarationHir(context, declarationId);
    final isInsideTrait =
        declarationId.hasParent && declarationId.parent.isTrait;

    if (isInsideTrait && function.isStatic) {
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
      ..static = function.isStatic
      ..returns = compileType(context, function.returnType)
      ..name = function.name
      ..optionalParameters
          .addAll(compileParameters(context, function.parameters))
      ..body = compileBody(context, declarationId).valueOrNull);
  },
);

Iterable<dart.Parameter> compileParameters(
  QueryContext context,
  List<ValueParameter> parameters,
) {
  return parameters.map((p) => dart.Parameter((b) => b
    ..named = true
    ..annotations.add(dart.refer('required', packageMetaUrl))
    ..type = compileType(context, p.type)
    ..name = p.name));
}
