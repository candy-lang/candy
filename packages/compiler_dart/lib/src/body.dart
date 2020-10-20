import 'dart:io';

import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:strings/strings.dart' as strings;

import 'declarations/module.dart';
import 'type.dart';

final compilePropertyInitializer = Query<DeclarationId, Option<dart.Code>>(
  'dart.compilePropertyInitializer',
  evaluateAlways: true,
  provider: (context, declarationId) {
    assert(declarationId.isProperty);
    final hir = getPropertyDeclarationHir(context, declarationId);
    if (hir.initializer == null) return None();

    return Some(_compileExpression(context, hir.initializer).code);
  },
);
final compileBody = Query<DeclarationId, Option<dart.Code>>(
  'dart.compileBody',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final body = getBody(context, declarationId);
    if (body.isNone) return None();
    final expressions = body.value;

    final visitor = DartExpressionVisitor(context);
    final compiled = expressions.expand((e) => e.accept(visitor));
    return Some(dart.Block((b) => b.statements.addAll(compiled)));
  },
);
final compileExpression = Query<Expression, dart.Expression>(
  'dart.compileExpression',
  evaluateAlways: true,
  provider: _compileExpression,
);

dart.Expression _compileExpression(
  QueryContext context,
  Expression expression,
) {
  final expressions = expression.accept(DartExpressionVisitor(context));
  assert(expressions.isNotEmpty);
  assert(expressions.last is dart.ToCodeExpression);

  return dart.Method(
          (b) => b..body = dart.Block((b) => b.statements.addAll(expressions)))
      .closure
      .call([], {}, []);
}

// dart.Expression _compileExpression(
//     QueryContext context, Expression expression) {
//   return expression.when(
//     identifier: (id, identifier) => identifier.when(
//       this_: () => dart.refer('this'),
//       super_: (_) => dart.refer('super'),
//       it: null,
//       field: null,
//       module: (id) => ModuleExpression(context, id),
//       trait: null,
//       class_: null,
//       parameter: null,
//       property: (target, name, _) {
//         final compiledTarget = _compileExpression(context, target);
//         if (compiledTarget is ModuleExpression) {
//           final currentModuleId =
//               declarationIdToModuleId(context, expression.id.declarationId);
//           if (compiledTarget.moduleId == currentModuleId) {
//             return dart.refer(name);
//           }

//           return dart.refer(
//             name,
//             moduleIdToImportUrl(context, compiledTarget.moduleId),
//           );
//         }
//         return compiledTarget.property(name);
//       },
//       localProperty: null,
//     ),
//     functionCall: null,
//     // functionCall: (id, target, arguments) {
//     //   final functionId = getPropertyIdentifierDeclarationId(
//     //     context,
//     //     target.identifier as PropertyIdentifier,
//     //   );
//     //   final parameters =
//     //       getFunctionDeclarationHir(context, functionId).parameters;
//     //   return dart.InvokeExpression.newOf(
//     //     _compileExpression(context, target),
//     //     [
//     //       for (final parameter in parameters)
//     //         _compileExpression(context, arguments[parameter.name]),
//     //     ],
//     //     {},
//     //     [],
//     //   );
//     // },
//     return_: (id, _, expression) {
//       // TODO(JonasWanke): non-local returns
//       return _compileExpression(context, expression).returned;
//     },
//   );
// }

class DartExpressionVisitor extends ExpressionVisitor<List<dart.Code>> {
  const DartExpressionVisitor(this.context) : assert(context != null);

  final QueryContext context;

  @override
  List<dart.Code> visitIdentifierExpression(IdentifierExpression node) =>
      node.accept(this);
  @override
  List<dart.Code> visitLiteralExpression(LiteralExpression node) {
    return node.literal.when(
      boolean: (value) => _saveSingle(node, dart.literalBool(value)),
      integer: (value) => _saveSingle(node, dart.literalNum(value)),
      string: (parts) {
        if (parts.isEmpty) return _saveSingle(node, dart.literalString(''));

        if (parts.length == 1 && parts.single is LiteralStringLiteralPart) {
          final part = parts.single as LiteralStringLiteralPart;
          return _saveSingle(
            node,
            dart.literalString(strings.escape(part.value)),
          );
        }

        final lowered = <dart.Code>[];
        for (final part in parts.whereType<InterpolatedStringLiteralPart>()) {
          lowered.addAll(part.value.accept(this));
        }

        final content = parts
            .map((p) => p.when(
                  literal: (value) => value,
                  interpolated: (expression) => '\$${_name(expression)}',
                ))
            .join();
        lowered.add(_save(node, dart.literalString(content)));

        return lowered;
      },
      lambda: (expressions, _) {
        final loweredExpressions = expressions.expand((e) => e.accept(this));
        final closure = dart.Method((b) => b
              ..body =
                  dart.Block((b) => b.statements.addAll(loweredExpressions)))
            .closure;
        return [_save(node, closure)];
      },
    );
  }

  @override
  List<dart.Code> visitCallExpression(CallExpression node) => [];
  @override
  List<dart.Code> visitFunctionCallExpression(FunctionCallExpression node) =>
      [];
  @override
  List<dart.Code> visitReturnExpression(ReturnExpression node) => [
        ...node.expression.accept(this),
        _refer(node.expression).returned.statement,
      ];

  @override
  List<dart.Code> visitThisIdentifier(ThisIdentifier node) => [];
  @override
  List<dart.Code> visitSuperIdentifier(SuperIdentifier node) => [];
  @override
  List<dart.Code> visitItIdentifier(ItIdentifier node) => [];
  @override
  List<dart.Code> visitFieldIdentifier(FieldIdentifier node) => [];
  @override
  List<dart.Code> visitModuleIdentifier(ModuleIdentifier node) => [];
  @override
  List<dart.Code> visitTraitIdentifier(TraitIdentifier node) => [];
  @override
  List<dart.Code> visitClassIdentifier(ClassIdentifier node) => [];
  @override
  List<dart.Code> visitParameterIdentifier(ParameterIdentifier node) => [];
  @override
  List<dart.Code> visitPropertyIdentifier(PropertyIdentifier node) => [];
  @override
  List<dart.Code> visitLocalPropertyIdentifier(LocalPropertyIdentifier node) =>
      [];

  String _name(Expression expression) => '_${expression.id.value}';
  dart.Expression _refer(Expression expression) =>
      dart.refer(_name(expression));
  dart.Code _save(Expression source, dart.Expression lowered) {
    return lowered
        .assignFinal(_name(source), compileType(context, source.type))
        .statement;
  }

  List<dart.Code> _saveSingle(Expression source, dart.Expression lowered) =>
      [_save(source, lowered)];
}

class ModuleExpression extends dart.InvokeExpression {
  ModuleExpression(QueryContext context, this.moduleId)
      : assert(context != null),
        assert(moduleId != null),
        super.constOf(
          compileType(context, CandyType.moduleDeclaration),
          [dart.literalString(moduleId.toString())],
          {},
          [],
        );

  final ModuleId moduleId;
}
