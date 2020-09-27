import 'dart:io';

import 'compilation/ast_hir_lowering.dart';
import 'compilation/hir.dart' as hir;
import 'compilation/hir/ids.dart';
import 'compilation/ids.dart';
import 'errors.dart';
import 'query.dart';
import 'resource_provider.dart';

Future<void> compile(Directory directory) async {
  final config = QueryConfig(
    resourceProvider: ResourceProvider.default_(directory),
  );
  final context = config.createContext();

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

    if (possibleFunctions.isEmpty) {
      throw CompilerError.noMainFunction(
        'Main function not found.',
        location: ErrorLocation(
          context.callQuery(moduleIdToDeclarationId, moduleId).resourceId,
        ),
      );
    } else if (possibleFunctions.length > 1) {
      throw CompilerError.multipleMainFunctions(
        'Multiple main functions found.',
        location: ErrorLocation(
          context.callQuery(moduleIdToDeclarationId, moduleId).resourceId,
          context
              .callQuery(getFunctionDeclarationAst, possibleFunctions.first)
              .name
              .span,
        ),
      );
    }

    return possibleFunctions.single;
  },
);
