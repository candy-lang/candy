import 'dart:io';

import 'package:meta/meta.dart';

import 'build_artifacts.dart';
import 'compilation/ast.dart';
import 'compilation/ast_hir_lowering.dart';
import 'compilation/hir.dart' as hir;
import 'compilation/hir/ids.dart';
import 'constants.dart';
import 'errors.dart';
import 'query.dart';
import 'resource_provider.dart';
import 'utils.dart';

Future<void> compile({
  @required Directory candyDirectory,
  @required Directory projectDirectory,
}) async {
  final config = QueryConfig(
    resourceProvider: ResourceProvider.default_(
      candyDirectory: candyDirectory,
      projectDirectory: projectDirectory,
    ),
    buildArtifactManager: BuildArtifactManager(projectDirectory),
  );
  final context = config.createContext();

  final mainFunctionId = context.callQuery(getMainFunction, mainModuleId);
  print(mainFunctionId);
}

const mainFunctionName = 'main';
final getMainFunction = Query<ModuleId, DeclarationId>(
  'getMainFunction',
  provider: (context, moduleId) {
    final module = getModuleDeclarationHir(context, moduleId);

    final possibleFunctions =
        module.innerDeclarationIds.where((id) => id.isFunction).where((id) {
      final function = getFunctionDeclarationHir(context, id);
      if (function.name != mainFunctionName) return false;
      if (function.valueParameters.length > 1) return false;
      if (function.valueParameters.length == 1 &&
          function.valueParameters.single.type !=
              hir.CandyType.list(hir.CandyType.string)) {
        return false;
      }
      return true;
    }).toList();

    if (possibleFunctions.isEmpty) {
      throw CompilerError.noMainFunction(
        'Main function not found.',
        location: ErrorLocation(
          moduleIdToDeclarationId(context, moduleId).resourceId,
        ),
      );
    } else if (possibleFunctions.length > 1) {
      final resourceId = moduleIdToDeclarationId(context, moduleId).resourceId;
      throw CompilerError.multipleMainFunctions(
        'Multiple main functions found.',
        location: ErrorLocation(
          resourceId,
          getFunctionDeclarationAst(context, possibleFunctions.first).name.span,
        ),
        relatedInformation: [
          for (final declarationId in possibleFunctions.skip(1))
            ErrorRelatedInformation(
              location: ErrorLocation(
                resourceId,
                getFunctionDeclarationAst(context, declarationId).name.span,
              ),
              message: 'Another function with a matching signature.',
            ),
        ],
      );
    }

    return possibleFunctions.single;
  },
);

final calculateFullHir = Query<ResourceId, Unit>(
  'calculateFullHir',
  provider: (context, resourceId) {
    _compileFull(context, DeclarationId(resourceId));
    return Unit();
  },
);

void _compileFull(QueryContext context, DeclarationId id) {
  List<DeclarationId> nextIds;
  if (id.isModule) {
    nextIds =
        getModuleDeclarationHir(context, declarationIdToModuleId(context, id))
            .innerDeclarationIds;
  } else if (id.isTrait) {
    nextIds = getTraitDeclarationHir(context, id).innerDeclarationIds;
  } else if (id.isImpl) {
    nextIds = getImplDeclarationHir(context, id).innerDeclarationIds;
  } else if (id.isClass) {
    nextIds = getClassDeclarationHir(context, id).innerDeclarationIds;
  } else if (id.isConstructor) {
    getConstructorDeclarationHir(context, id);
  } else if (id.isFunction) {
    getFunctionDeclarationHir(context, id);
    getBody(context, id);
  } else if (id.isProperty) {
    getPropertyDeclarationHir(context, id);
    getBody(context, id);
  } else {
    assert(false);
  }

  for (final id in nextIds.orEmpty) {
    _compileFull(context, id);
  }
}
