import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';
import '../utils.dart';

final compileProperty = Query<DeclarationId, dart.Field>(
  'dart.compileProperty',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // This is only for global properties and those in classes, not for
    // properties in traits as they create getter/setter methods.
    final propertyHir = getPropertyDeclarationHir(context, declarationId);
    final isInsideClass =
        declarationId.hasParent && declarationId.parent.isClass;

    return dart.Field((b) => b
      ..static = propertyHir.isStatic && declarationId.parent.isNotModule
      ..modifier = propertyHir.isMutable
          ? dart.FieldModifier.var$
          : dart.FieldModifier.final$
      ..name = mangleName(propertyHir.name)
      ..type = compileType(context, propertyHir.type)
      // In classes, the constructor is reponsible for handling defaults.
      ..assignment = isInsideClass && !propertyHir.isStatic
          ? null
          : compilePropertyInitializer(context, declarationId).valueOrNull);
  },
);
