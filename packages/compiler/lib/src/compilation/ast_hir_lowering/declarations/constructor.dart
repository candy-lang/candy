import '../../../query.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import 'class.dart';
import 'property.dart';

extension ConstructorDeclarationId on DeclarationId {
  bool get isConstructor =>
      path.isNotEmpty && path.last.data is ConstructorDeclarationPathData;
  bool get isNotConstructor => !isConstructor;
}

final getConstructorDeclarationHir =
    Query<DeclarationId, hir.ConstructorDeclaration>(
  'getConstructorDeclarationHir',
  provider: (context, declarationId) {
    final classId = declarationId.parent;
    assert(classId.isClass);

    final classHir = getClassDeclarationHir(context, classId);
    final propertyIds =
        classHir.innerDeclarationIds.where((id) => id.isProperty);

    return hir.ConstructorDeclaration(
      typeParameters: classHir.typeParameters,
      valueParameters: propertyIds
          .map((id) => getPropertyDeclarationHir(context, id))
          .where((p) => !p.isStatic)
          .map((property) => hir.ValueParameter(
                name: property.name,
                type: property.type,
                defaultValue: property.initializer,
              ))
          .toList(),
    );
  },
);
