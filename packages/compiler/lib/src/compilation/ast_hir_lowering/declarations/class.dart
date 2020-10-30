import 'package:parser/parser.dart' as ast;

import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'declarations.dart';
import 'module.dart';
import 'property.dart';

extension ClassDeclarationId on DeclarationId {
  bool get isClass =>
      path.isNotEmpty && path.last.data is ClassDeclarationPathData;
  bool get isNotClass => !isClass;
}

final getClassDeclarationAst = Query<DeclarationId, ast.ClassDeclaration>(
  'getClassDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isClass);

    final declaration = context.callQuery(getDeclarationAst, declarationId);
    assert(declaration is ast.ClassDeclaration, 'Wrong return type.');
    return declaration as ast.ClassDeclaration;
  },
);
final getClassDeclarationHir = Query<DeclarationId, hir.ClassDeclaration>(
  'getClassDeclarationHir',
  provider: (context, declarationId) {
    final classAst = getClassDeclarationAst(context, declarationId);
    final name = classAst.name.name;

    return hir.ClassDeclaration(
      name: name,
      thisType: createClassThisType(context, declarationId),
      // ignore: can_be_null_after_null_aware
      typeParameters: classAst.typeParameters?.parameters.orEmpty
          .map((p) => hir.TypeParameter(
                name: p.name.name,
                upperBound: p.bound != null
                    ? astTypeToHirType(context, Tuple2(declarationId, p.bound))
                    : hir.CandyType.any,
              ))
          .toList(),
      innerDeclarationIds: getInnerDeclarationIds(context, declarationId) +
          [declarationId.inner(DeclarationPathData.constructor())],
      syntheticImpls: classAst.isData
          ? generateSyntheticDataClassImpls(context, declarationId)
          : [],
    );
  },
);

final generateSyntheticDataClassImpls =
    Query<DeclarationId, List<hir.SyntheticImpl>>(
  'generateSyntheticDataClassImpls',
  provider: (context, classId) {
    final properties = getInnerDeclarationIds(context, classId)
        .where((it) => it.isProperty)
        .map((it) => getPropertyDeclarationHir(context, it))
        .toList();
    return [
      _generateEqualsImpl(
        context,
        createClassThisType(context, classId),
        classId.inner(DeclarationPathData.impl('synthetic')),
        properties,
      ),
    ];
  },
);

final createClassThisType = Query<DeclarationId, hir.UserCandyType>(
  'createClassThisType',
  provider: (context, classId) {
    final classAst = getClassDeclarationAst(context, classId);
    final name = classAst.name.name;
    return hir.UserCandyType(
      declarationIdToModuleId(context, classId).parent,
      name,
      // ignore: can_be_null_after_null_aware
      arguments: classAst.typeParameters?.parameters.orEmpty
          .map((p) => hir.CandyType.parameter(p.name.name, classId))
          .toList(),
    );
  },
);

hir.SyntheticImpl _generateEqualsImpl(
  QueryContext context,
  hir.UserCandyType thisType,
  DeclarationId implId,
  List<hir.PropertyDeclaration> properties,
) {
  final methodId = implId.inner(DeclarationPathData.function('equals'));

  var nextId = 0;
  DeclarationLocalId id() => DeclarationLocalId(methodId, nextId++);
  final parameterOther = hir.Identifier.parameter(
    id(),
    'other',
    thisType,
  );
  final invalidId = DeclarationLocalId(methodId, -1);

  return hir.SyntheticImpl(
    implHir: hir.ImplDeclaration(
      type: thisType,
      traits: [hir.CandyType.equals],
      innerDeclarationIds: [methodId],
    ),
    methods: [
      Tuple2(
        hir.FunctionDeclaration(
          isStatic: false,
          name: 'equals',
          valueParameters: [
            hir.ValueParameter(name: 'other', type: hir.CandyType.this_()),
          ],
          returnType: hir.CandyType.bool,
        ),
        [
          hir.Expression.return_(
            id(),
            invalidId,
            properties
                .map(
                  (it) {
                    hir.Expression create(
                      hir.Identifier receiver,
                    ) {
                      return hir.Expression.identifier(
                        id(),
                        hir.PropertyIdentifier(
                          it.id,
                          it.type.bakeThisType(thisType),
                          isMutable: it.isMutable,
                          receiver: hir.Expression.identifier(id(), receiver),
                        ),
                      );
                    }

                    return Tuple2(
                      create(hir.Identifier.this_(thisType)),
                      create(parameterOther),
                    );
                  },
                )
                .map(
                  (it) => hir.Expression.functionCall(
                    id(),
                    hir.Expression.identifier(
                      id(),
                      hir.Identifier.property(
                        moduleIdToDeclarationId(
                          context,
                          hir.CandyType.equals.virtualModuleId,
                        ).inner(DeclarationPathData.function('equals')),
                        hir.CandyType.function(
                          receiverType: it.first.type,
                          parameterTypes: [it.first.type],
                          returnType: hir.CandyType.bool,
                        ),
                        isMutable: false,
                        receiver: it.first,
                      ),
                    ),
                    {'other': it.second},
                  ),
                )
                .reduce(
                  (value, element) => hir.Expression.functionCall(
                    id(),
                    hir.Expression.identifier(
                      id(),
                      hir.Identifier.property(
                        moduleIdToDeclarationId(
                          context,
                          hir.CandyType.and.virtualModuleId,
                        ).inner(DeclarationPathData.function('and')),
                        hir.CandyType.function(
                          receiverType: value.type,
                          parameterTypes: [value.type],
                          returnType: hir.CandyType.bool,
                        ),
                        isMutable: false,
                        receiver: value,
                      ),
                    ),
                    {'other': element},
                  ),
                ),
          ),
        ],
      ),
    ],
  );
}
