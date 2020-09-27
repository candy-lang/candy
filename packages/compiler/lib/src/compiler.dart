import 'dart:io';

import 'compilation/ast_hir_lowering.dart';
import 'compilation/hir.dart' as hir;
import 'compilation/hir/ids.dart';
import 'compilation/ids.dart';
import 'query.dart';
import 'resource_provider.dart';

Future<void> compile(Directory directory) async {
  final context = QueryContext(
    resourceProvider: ResourceProvider.default_(directory),
  );

  final mainFunctionId =
      context.callQuery(getMainFunction, ModuleId(PackageId.this_, ['main']));
  print(mainFunctionId);
}

const mainFunctionName = 'main';
final getMainFunction = Query<ModuleId, DeclarationId>(
  'getMainFunction',
  provider: (context, moduleId) {
    final module = context.callQuery(getModuleDeclarationHir, moduleId);

    final possibleFunctions =
        module.innerDeclarationIds.where((id) => id.isFunction).where((id) {
      final function = context.callQuery(getFunctionDeclarationHir, id);
      if (function.name != mainFunctionName) return false;
      if (function.parameters.length > 1) return false;
      if (function.parameters.length == 1 &&
          function.parameters.single.type !=
              hir.CandyType.list(hir.CandyType.string)) {
        return false;
      }
      return true;
    }).toList();
    assert(possibleFunctions.isNotEmpty, 'Main function not found.');
    assert(possibleFunctions.length <= 1, 'Multiple main functions found.');

    return possibleFunctions.single;
  },
);
