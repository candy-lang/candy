import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';
import 'declaration.dart';
import 'module.dart';

final compileFunction = Query<DeclarationId, dart.Method>(
  'dart.compileFunction',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final functionHir = getFunctionDeclarationHir(context, declarationId);
    final moduleId = declarationIdToModuleId(context, declarationId);

    dart.Code body;
    if (moduleId == ModuleId.coreCollections.nested(['list', 'List']) &&
        declarationId.simplePath.last.nameOrNull.startsWith('of')) {
      body = _compileListOf(context, functionHir);
    } else if (moduleId ==
            ModuleId.coreCollections.nested(['list', 'array', 'ArrayList']) &&
        declarationId.simplePath.last.nameOrNull.startsWith('of')) {
      body = _compileArrayListOf(functionHir);
    }
    body ??= compileBody(context, declarationId).valueOrNull;

    var name = functionHir.name;
    // TODO(JonasWanke): make this safer
    if (name == 'equals') name = 'operator ==';

    return dart.Method((b) => b
      ..static = functionHir.isStatic && declarationId.parent.isNotModule
      ..returns = compileType(context, functionHir.returnType)
      ..name = name
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

dart.Code _compileListOf(
  QueryContext context,
  FunctionDeclaration functionHir,
) {
  final itemType = functionHir.valueParameters.first.type;
  return dart
      .refer(
        'ArrayList',
        moduleIdToImportUrl(context, CandyType.arrayListModuleId.parent),
      )
      .property(functionHir.name)
      .call(
    functionHir.valueParameters.map((p) => dart.refer(p.name)).toList(),
    {},
    [compileType(context, itemType)],
  ).code;
}

dart.Code _compileArrayListOf(FunctionDeclaration functionHir) {
  final list = dart.literalList(
    functionHir.valueParameters.map((p) => p.name).map(dart.refer),
    dart.refer(functionHir.typeParameters.single.name),
  );

  return dart.refer('ArrayList<Item>').call([list], {}, []).code;
}
