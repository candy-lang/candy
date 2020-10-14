import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../constants.dart';
import 'function.dart';
import 'property.dart';

final compileClass = Query<DeclarationId, dart.Class>(
  'dart.compileClass',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // ignore: non_constant_identifier_names
    final class_ = getClassDeclarationHir(context, declarationId);

    final properties = class_.innerDeclarationIds
        .where((id) => id.isProperty)
        .map((id) => compileProperty(context, id));
    final methods = class_.innerDeclarationIds
        .where((id) => id.isFunction)
        .map((id) => compileFunction(context, id));
    return dart.Class((b) => b
      ..annotations.add(dart.refer('sealed', packageMetaUrl))
      ..name = class_.name
      ..fields.addAll(properties)
      ..methods.addAll(methods));
  },
);
