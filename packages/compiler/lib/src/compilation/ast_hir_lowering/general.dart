import 'package:parser/parser.dart' as ast;
import 'package:dartx/dartx.dart';

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
    final resourceId = inputs.first;
    final moduleName = inputs.second;
    final moduleNameSpan = inputs.third;
    final onlySearchPublic = inputs.fourth;

    var useLines = lowerUseLinesAstToHir(context, resourceId);
    if (onlySearchPublic) useLines = useLines.where((u) => u.isPublic).toList();
    if (useLines.isEmpty) return Option.none();

    final matches = useLines.flatMap((useLine) {
      final declarationId = moduleIdToDeclarationId(context, useLine.moduleId);
      assert(declarationId.path.isEmpty);

      // TODO(JonasWanke): Ignore non-public declarations.
      final directMatches = getInnerDeclarationIds(context, declarationId)
          .where((id) => id.simplePath.first.nameOrNull == moduleName);
      if (directMatches.isNotEmpty) {
        return directMatches.map((d) => declarationIdToModuleId(context, d));
      }

      return findModuleInUseLines(
        context,
        Tuple4(declarationId.resourceId, moduleName, moduleNameSpan, true),
      ).toList();
    });

    if (matches.length > 1) {
      throw CompilerError.ambiguousExpression(
        'Identifier `$moduleName` found in multiple places.',
        location: ErrorLocation(resourceId, moduleNameSpan),
        relatedInformation: matches.map((match) {
          final declarationId = moduleIdToDeclarationId(context, match);
          final ast = getDeclarationAst(context, declarationId);
          return ErrorRelatedInformation(
            location:
                ErrorLocation(declarationId.resourceId, ast.representativeSpan),
            message: 'A declaration with a matching name.',
          );
        }).toList(),
      );
    } else if (matches.isNotEmpty) {
      return Option.some(matches.single);
    } else {
      return Option.none();
    }
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
    if (modules.any((m) => m.moduleId.packageId == PackageId.core)) {
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
        PackageId.this_,
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
