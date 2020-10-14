import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';

final compileProperty = Query<DeclarationId, dart.Field>(
  'dart.compileProperty',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final property = getPropertyDeclarationHir(context, declarationId);
    final isInsideClass =
        declarationId.hasParent && declarationId.parent.isClass;

    return dart.Field((b) => b
      ..static = property.isStatic
      ..modifier = property.isMutable
          ? dart.FieldModifier.var$
          : dart.FieldModifier.final$
      ..name = property.name
      ..type = compileType(context, property.type)
      // In classes, the constructor is reponsible for handling defaults.
      ..assignment = !isInsideClass && property.initializer != null
          ? compilePropertyInitializer(context, declarationId)
          : null);
  },
);
