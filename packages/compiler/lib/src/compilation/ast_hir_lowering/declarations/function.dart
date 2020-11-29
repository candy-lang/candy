import 'package:parser/parser.dart' as ast;

import '../../../errors.dart';
import '../../../query.dart';
import '../../../utils.dart';
import '../../hir.dart' as hir;
import '../../hir/ids.dart';
import '../type.dart';
import 'class.dart';
import 'declarations.dart';
import 'impl.dart';
import 'module.dart';

extension FunctionDeclarationId on DeclarationId {
  bool get isFunction =>
      path.isNotEmpty && path.last.data is FunctionDeclarationPathData;
  bool get isNotFunction => !isFunction;
}

final getFunctionDeclarationAst = Query<DeclarationId, ast.FunctionDeclaration>(
  'getFunctionDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.isFunction);

    final declaration = getDeclarationAst(context, declarationId);
    assert(declaration is ast.FunctionDeclaration, 'Wrong return type.');
    return declaration as ast.FunctionDeclaration;
  },
);
final getFunctionDeclarationHir = Query<DeclarationId, hir.FunctionDeclaration>(
  'getFunctionDeclarationHir',
  provider: (context, declarationId) {
    if (!doesDeclarationExist(context, declarationId)) {
      return getSyntheticMethod(context, declarationId).first;
    }

    final functionAst = getFunctionDeclarationAst(context, declarationId);

    if (functionAst.isTest) {
      if (functionAst.typeParameters != null &&
          functionAst.typeParameters.parameters.isNotEmpty) {
        throw CompilerError.invalidTypeParameterInTestFun(
          'Test functions may not be declared with type parameters.',
          location: ErrorLocation(
            declarationId.resourceId,
            functionAst.typeParameters.parameters.first.span,
          ),
        );
      }
      if (functionAst.valueParameters.isNotEmpty) {
        throw CompilerError.invalidValueParameterInTestFun(
          'Test functions may not be declared with parameters.',
          location: ErrorLocation(
            declarationId.resourceId,
            functionAst.valueParameters.first.span,
          ),
        );
      }
    }

    // ignore: can_be_null_after_null_aware
    final typeParameters = functionAst.typeParameters?.parameters.orEmpty
        .map((p) => hir.TypeParameter(
              name: p.name.name,
              upperBound: p.bound != null
                  ? astTypeToHirType(
                      context,
                      Tuple2(declarationId.parent, p.bound),
                    )
                  : hir.CandyType.any,
            ))
        .toList();

    return hir.FunctionDeclaration(
      isStatic: functionAst.isStatic ||
          functionAst.isTest ||
          declarationId.parent.isModule,
      isTest: functionAst.isTest,
      name: functionAst.name.name,
      typeParameters: typeParameters,
      valueParameters: functionAst.valueParameters
          .map((p) => hir.ValueParameter(
                name: p.name.name,
                type: astTypeToHirType(context, Tuple2(declarationId, p.type)),
              ))
          .toList(),
      returnType: functionAst.returnType != null
          ? astTypeToHirType(
              context,
              Tuple2(declarationId, functionAst.returnType),
            )
          : hir.CandyType.unit,
    );
  },
);

Tuple2<hir.FunctionDeclaration, List<hir.Expression>> getSyntheticMethod(
  QueryContext context,
  DeclarationId declarationId,
) {
  final implId = declarationId.parent;
  assert(implId.isImpl);

  if (declarationId.simplePath.last.nameOrNull == 'randomSample') {
    return Tuple2(
      hir.FunctionDeclaration(
        isStatic: true,
        isTest: false,
        name: 'randomSample',
        valueParameters: [
          hir.ValueParameter(name: 'source', type: hir.CandyType.randomSource),
        ],
        returnType: getImplDeclarationHir(context, implId).type,
      ),
      [],
    );
  }

  final classId = implId.parent;
  assert(classId.isClass);
  final classHir = getClassDeclarationHir(context, classId);

  final implIndex = implId.path.last.disambiguator;
  final syntheticImpl = classHir.syntheticImpls[implIndex];
  final methodName = declarationId.simplePath.last.nameOrNull;
  return syntheticImpl.methods.singleWhere((it) => it.first.name == methodName);
}
