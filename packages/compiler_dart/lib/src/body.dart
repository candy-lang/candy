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
  List<dart.Code> visitIdentifierExpression(IdentifierExpression node) {
    List<dart.Code> referTraitOrClass(DeclarationId id) {
      final importUrl = declarationIdToImportUrl(context, id);
      final refer = dart.refer(id.simplePath.last.nameOrNull, importUrl);
      return _saveSingle(node, refer);
    }

    return node.identifier.when(
      this_: () => _saveSingle(node, dart.refer('this')),
      super_: null,
      module: null,
      trait: referTraitOrClass,
      class_: referTraitOrClass,
      parameter: (id, name, _) {
        if (name == 'this') {
          final expression = getExpression(context, id);

          if (expression is Some &&
              expression.value is LiteralExpression &&
              (expression.value as LiteralExpression).literal
                  is LambdaLiteral) {
            return _saveSingle(
              node,
              dart.refer(
                _lambdaThisName(expression.value as LiteralExpression),
              ),
            );
          }
        }
        return _saveSingle(node, dart.refer(name));
      },
      property: (id, _, target) {
        final name = id.simplePath.last.nameOrNull;

        // Generated Dart-functions always use named arguments, which our type
        // system can't express. Hence we don't manually add the type in this
        // case.
        final explicitType = id.isNotFunction;

        return [
          if (target != null) ...[
            ...target.accept(this),
            _save(
              node,
              _refer(target.id).property(name),
              explicitType: explicitType,
            ),
          ] else
            _save(
              node,
              dart.refer(name, declarationIdToImportUrl(context, id.parent)),
              explicitType: explicitType,
            ),
        ];
      },
      localProperty: (id, _, __) => _saveSingle(node, _refer(id)),
    );
  }

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
                  interpolated: (expression) => '\$${_name(expression.id)}',
                ))
            .join();
        lowered.add(_save(node, dart.literalString(content)));

        return lowered;
      },
      lambda: (parameters, expressions, returnType, receiverType) {
        final closure = dart.Method((b) {
          if (receiverType != null) {
            b.requiredParameters
                .add(dart.Parameter((b) => b..name = _lambdaThisName(node)));
          }

          final params = parameters.map((p) => dart.Parameter((b) => b
            ..type = compileType(context, p.type)
            ..name = p.name));
          b.requiredParameters.addAll(params);

          final loweredExpressions = expressions.expand((e) => e.accept(this));
          b.body = dart.Block((b) => b.statements.addAll(loweredExpressions));
        }).closure;
        return [_save(node, closure)];
      },
    );
  }

  String _lambdaThisName(LiteralExpression lambdaExpression) =>
      '${_name(lambdaExpression.id)}_this';

  @override
  List<dart.Code> visitPropertyExpression(PropertyExpression node) {
    return [
      ...node.initializer.accept(this),
      _save(node, _refer(node.initializer.id), isMutable: node.isMutable),
    ];
  }

  @override
  List<dart.Code> visitNavigationExpression(NavigationExpression node) => [];
  @override
  List<dart.Code> visitCallExpression(CallExpression node) => [];
  @override
  List<dart.Code> visitFunctionCallExpression(FunctionCallExpression node) {
    final arguments = {
      for (final entry in node.valueArguments.entries)
        entry.key: _refer(entry.value.id),
    };
    return [
      ...node.target.accept(this),
      for (final argument in node.valueArguments.values)
        ...argument.accept(this),
      _save(node, _refer(node.target.id).call([], arguments, [])),
    ];
  }

  @override
  List<dart.Code> visitReturnExpression(ReturnExpression node) => [
        // TODO(JonasWanke): support labeled returns
        if (node.expression != null) ...[
          ...node.expression.accept(this),
          _refer(node.expression.id).returned.statement,
        ] else
          dart.Code('return;'),
      ];

  @override
  List<dart.Code> visitLoopExpression(LoopExpression node) => [
        if (node.type != CandyType.unit)
          dart.literalNull.assignVar(_name(node.id)).statement,
        dart.Code('${_label(node.id)}:\nwhile (true) {'),
        for (final expression in node.body) ...expression.accept(this),
        dart.Code('}'),
      ];

  @override
  List<dart.Code> visitBreakExpression(BreakExpression node) => [
        if (node.expression != null) ...[
          ...node.expression.accept(this),
          _refer(node.scopeId).assign(_refer(node.expression.id)).statement,
        ],
        dart.Code('break ${_label(node.scopeId)};'),
      ];
  @override
  List<dart.Code> visitContinueExpression(ContinueExpression node) => [
        dart.Code('continue ${_label(node.scopeId)};'),
      ];

  String _name(DeclarationLocalId id) => '_${id.value}';
  dart.Expression _refer(DeclarationLocalId id) => dart.refer(_name(id));
  dart.Code _save(
    Expression source,
    dart.Expression lowered, {
    bool explicitType = true,
    bool isMutable = false,
  }) {
    final type = explicitType ? compileType(context, source.type) : null;

    if (isMutable) {
      return lowered.assignVar(_name(source.id), type).statement;
    } else {
      return lowered.assignFinal(_name(source.id), type).statement;
    }
  }

  List<dart.Code> _saveSingle(Expression source, dart.Expression lowered) =>
      [_save(source, lowered)];

  String _label(DeclarationLocalId id) => '_label_${id.value}';
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
