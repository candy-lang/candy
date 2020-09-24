import 'package:dartx/dartx.dart';

import '../../constants.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast/module.dart';
import '../ast/parser.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';

const moduleFileName = 'module$candyFileExtension';

extension on ResourceId {
  bool get isModuleFile => fileName == moduleFileName;
}

/// Resolves a module given a base [ResourceId] and a [String] as used in a
/// use-line.
final resolveModule = Query<Tuple2<ResourceId, String>, DeclarationId>(
  'resolveModule',
  provider: (context, inputs) {
    final resourceId = inputs.first;
    final modulePath = inputs.second;

    // TODO(JonasWanke): nested module paths
    // TODO(JonasWanke): parent module
    // TODO(JonasWanke): packages
    final siblingModuleId =
        resourceId.sibling('$modulePath$candyFileExtension');
    if (context.callQuery(doesResourceExist, siblingModuleId)) {
      return DeclarationId(siblingModuleId, []);
    }

    final nestedModuleId = resourceId.sibling('$modulePath/$moduleFileName');
    if (context.callQuery(doesResourceExist, nestedModuleId)) {
      return DeclarationId(nestedModuleId, []);
    }

    return null;
  },
);

final getModuleDeclarationHir = Query<DeclarationId, hir.ModuleDeclaration>(
  'getModuleDeclarationHir',
  provider: (context, declarationId) {
    final ast = context.callQuery(getModuleDeclarationAst, declarationId);
    return hir.ModuleDeclaration(
      parent: context.callQuery(getParentDeclarationId, declarationId),
      name: ast.name.name,
      // TODO(JonasWanke): child declarations
    );
  },
);

final getParentDeclarationId = Query<DeclarationId, DeclarationId>(
  'getParentDeclarationId',
  provider: (context, declarationId) {
    assert(declarationId.isModule);

    if (declarationId.path.isNotEmpty) {
      // Outer declaration in the same file (or just that file).
      return DeclarationId(
        declarationId.resourceId,
        declarationId.path.dropLast(1),
      );
    } else {
      // Declaration potentially in parent file.
      return context.callQuery(
        resolveModule,
        Tuple2(declarationId.resourceId, 'super'),
      );
    }
  },
);
