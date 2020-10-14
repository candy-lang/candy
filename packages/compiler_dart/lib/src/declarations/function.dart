import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';

final compileFunction = Query<DeclarationId, dart.Method>(
  'dart.compileFunction',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final function = getFunctionDeclarationHir(context, declarationId);
    return dart.Method((b) => b
      ..static = function.isStatic
      ..name = function.name
      ..returns = compileType(context, function.returnType)
      ..body = compileBody(context, declarationId));
  },
);
