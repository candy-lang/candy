import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast/parser.dart';
import '../ast_hir_lowering.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';
import '../ids.dart';

final Query<Tuple4<ResourceId, String, ast.SourceSpan, bool>, Option<ModuleId>>
    findModuleInUseLines =
    Query<Tuple4<ResourceId, String, ast.SourceSpan, bool>, Option<ModuleId>>(
  'findModuleInUseLines',
  provider: (context, inputs) {
    final results = findIdentifierInUseLines(
      context,
      Tuple4(inputs.first, inputs.second, inputs.fourth, true),
    );

    if (results.isEmpty) return None();
    if (results.length > 1) {
      throw CompilerError.ambiguousExpression(
        'Identifier `${inputs.second}` found in multiple places.',
        location: ErrorLocation(inputs.first, inputs.third),
        relatedInformation: results.map((declarationId) {
          final ast = getDeclarationAst(context, declarationId);
          return ErrorRelatedInformation(
            location:
                ErrorLocation(declarationId.resourceId, ast.representativeSpan),
            message: 'A declaration with a matching name.',
          );
        }).toList(),
      );
    }
    return Some(declarationIdToModuleId(context, results.single));
  },
);

final Query<Tuple4<ResourceId, String, bool, bool>, List<DeclarationId>>
    findIdentifierInUseLines =
    Query<Tuple4<ResourceId, String, bool, bool>, List<DeclarationId>>(
  'findIdentifierInUseLines',
  provider: (context, inputs) {
    final resourceId = inputs.first;
    final identifier = inputs.second;
    final onlySearchPublic = inputs.third;
    final onlyFindModules = inputs.fourth;

    var useLines = lowerUseLinesAstToHir(context, resourceId);
    if (onlySearchPublic) useLines = useLines.where((u) => u.isPublic).toList();
    if (useLines.isEmpty) return [];

    final matches = useLines.flatMap((useLine) {
      final declarationId = moduleIdToDeclarationId(context, useLine.moduleId);
      assert(declarationId.path.isEmpty);

      // TODO(JonasWanke): Ignore non-public declarations.
      final directMatches = getInnerDeclarationIds(context, declarationId)
          .where((id) =>
              !onlyFindModules || id.isModule || id.isTrait || id.isClass)
          .where((id) => id.simplePath.first.nameOrNull == identifier);
      if (directMatches.isNotEmpty) {
        return directMatches;
      }

      return findIdentifierInUseLines(
        context,
        inputs.copyWith(first: declarationId.resourceId, third: true),
      );
    });

    return matches.toList();
  },
);

final lowerUseLinesAstToHir = Query<ResourceId, List<hir.UseLine>>(
  'lowerUseLinesAstToHir',
  provider: (context, resourceId) {
    // TODO(JonasWanke): packages with slashes

    final useLines = getAst(context, resourceId).useLines;
    var modules = useLines
        .map((l) => lowerUseLineAstToHir(context, Tuple2(resourceId, l)))
        .toList();
    if (modules.none((m) => m.moduleId.packageId == PackageId.core)) {
      modules = modules + [hir.UseLine(ModuleId.core, isPublic: false)];
    }

    return modules;
  },
);

/// Resolves a module given a base [ResourceId] and an [ast.UseLine].
final lowerUseLineAstToHir =
    Query<Tuple2<ResourceId, ast.UseLine>, hir.UseLine>(
  'lowerUseLineAstToHir',
  provider: (context, inputs) {
    final resourceId = inputs.first;
    final useLine = inputs.second;
    // TODO(JonasWanke): packages with slashes

    final moduleId = useLine.map(
      localAbsolute: (useLine) => ModuleId(
        resourceId.packageId,
        useLine.pathSegments.map((s) => s.name).toList(),
      ),
      localRelative: (useLine) {
        var resolved = resourceIdToModuleId(context, resourceId);
        assert(useLine.leadingDots.isNotEmpty);
        for (var i = 0; i < useLine.leadingDots.length - 1; i++) {
          if (resolved == null) {
            throw CompilerError.invalidUseLine(
              'This use line uses too many dots.',
              location: ErrorLocation(resourceId, useLine.span),
            );
          }
          resolved = resolved.parent;
        }
        return resolved
            .nested(useLine.pathSegments.map((s) => s.name).toList());
      },
      global: (useLine) {
        if (useLine.moduleName != null) {
          throw CompilerError.unsupportedFeature(
            'Module imports from other packages are not yet supported.',
          );
        }
        if (useLine.packagePathSegments.length > 1) {
          throw CompilerError.unsupportedFeature(
            'Scoped packages are not yet supported.',
          );
        }

        return ModuleId(PackageId(useLine.packagePathSegments.single.name), []);
      },
    );
    return hir.UseLine(moduleId, isPublic: useLine.isPublic);
  },
);
