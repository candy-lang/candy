import 'dart:io';

import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:strings/strings.dart' as strings;

import 'declarations/declaration.dart';
import 'declarations/module.dart';
import 'type.dart';

final compilePropertyInitializer = Query<DeclarationId, Option<dart.Code>>(
  'dart.compilePropertyInitializer',
  evaluateAlways: true,
  provider: (context, declarationId) {
    assert(declarationId.isProperty);
    final hir = getPropertyDeclarationHir(context, declarationId);
    if (hir.initializer == null) return None();

    return Some(
        _compileExpression(context, declarationId, hir.initializer).code);
  },
);
final compileBody = Query<DeclarationId, Option<dart.Code>>(
  'dart.compileBody',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final body = getBody(context, declarationId);
    if (body.isNone) return None();
    final expressions = body.value;

    final visitor = DartExpressionVisitor(context, declarationId);
    final compiled = expressions.expand((e) => e.accept(visitor));
    return Some(dart.Block((b) => b.statements.addAll(compiled)));
  },
);
final compileExpression =
    Query<Tuple2<DeclarationId, Expression>, dart.Expression>(
  'dart.compileExpression',
  evaluateAlways: true,
  provider: (context, inputs) {
    final declarationId = inputs.first;
    final expression = inputs.second;
    return _compileExpression(context, declarationId, expression);
  },
);

dart.Expression _compileExpression(
  QueryContext context,
  DeclarationId declarationId,
  Expression expression,
) {
  final statements =
      expression.accept(DartExpressionVisitor(context, declarationId));
  assert(statements.isNotEmpty);
  assert(statements.last is dart.ToCodeExpression);

  return lambdaOf([
    ...statements,
    DartExpressionVisitor._refer(expression.id).returned.statement,
  ]).call([], {}, []);
}

class DartExpressionVisitor extends ExpressionVisitor<List<dart.Code>> {
  const DartExpressionVisitor(this.context, this.declarationId)
      : assert(context != null),
        assert(declarationId != null);

  final QueryContext context;
  final DeclarationId declarationId;
  ResourceId get resourceId => declarationId.resourceId;

  @override
  List<dart.Code> visitIdentifierExpression(IdentifierExpression node) {
    return node.identifier.when(
      this_: (_) => _saveSingle(node, dart.refer('this')),
      super_: (_) {
        throw CompilerError.internalError(
          '`super` is not yet supported in Dart compiler.',
        );
      },
      reflection: (id, __) {
        if (id.isModule) {
          throw CompilerError.internalError(
            'Reflection identifiers pointing to modules are not yet supported in Dart compiler.; `$id`',
          );
        } else if (id.isTrait || id.isClass || id.isProperty || id.isFunction) {
          dart.Expression expression;
          if (id.isProperty || id.isFunction) {
            assert(id.parent.isNotModule);

            final propertyType = compileType(
              context,
              id.isProperty
                  ? getPropertyDeclarationHir(context, id).type
                  : getFunctionDeclarationHir(context, id).returnType,
            );

            final propertyName = id.simplePath.last.nameOrNull;

            final valueParameters = id.isFunction
                ? getFunctionDeclarationHir(context, id).valueParameters
                : <ValueParameter>[];
            var body = dart.refer('instance').property(propertyName);
            if (id.isFunction) {
              body = body.call(
                [
                  for (final parameter in valueParameters)
                    dart.refer(parameter.name),
                ],
                {},
                [],
              );
            }

            expression = dart.Method((b) => b
              ..returns = propertyType
              ..requiredParameters.add(dart.Parameter((b) => b
                ..type = compileType(
                  context,
                  getPropertyDeclarationParentAsType(context, id).value,
                )
                ..name = 'instance'))
              ..requiredParameters
                  .addAll(valueParameters.map((p) => dart.Parameter((b) => b
                    ..type = compileType(context, p.type)
                    ..name = p.name)))
              ..body = body.code).closure;
          } else {
            expression = dart.refer(
              id.simplePath.last.nameOrNull,
              declarationIdToImportUrl(context, id.parent),
            );
          }
          return _saveSingle(node, expression);
        }
        throw CompilerError.internalError(
          'Invalid reflection target for Dart compiler: `$id`.',
        );
      },
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
      property: (id, type, _, __, receiver) {
        final name = id.simplePath.last.nameOrNull;

        if (receiver != null) {
          return [
            ...receiver.accept(this),
            _save(node, _refer(receiver.id).property(name)),
          ];
        }

        dart.Expression lowered;
        if ((id.isProperty || id.isFunction) && id.parent.isNotModule) {
          lowered = compileTypeName(context, id.parent).property(name);
        } else {
          lowered = dart.refer(
            id.simplePath.last.nameOrNull,
            declarationIdToImportUrl(context, id.parent),
          );
        }
        return _saveSingle(node, lowered);
      },
      localProperty: (id, _, __, ___) => _saveSingle(node, _refer(id)),
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
    final target = node.target;
    if (target is IdentifierExpression &&
        target.identifier is PropertyIdentifier) {
      final identifier = target.identifier as PropertyIdentifier;
      final parentModuleId =
          declarationIdToModuleId(context, identifier.id.parent);
      final methodName = identifier.id.simplePath.last.nameOrNull;

      List<dart.Code> lazyBoolExpression(
        String name,
        dart.Expression Function(dart.Expression left, dart.Expression right)
            binaryBuilder,
      ) {
        assert(name == methodName);

        final left = identifier.receiver;
        final right = node.valueArguments['other'];

        if (isAssignableTo(context, Tuple2(left.type, CandyType.bool))) {
          final rightCompiled = lambdaOf([
            ...right.accept(this),
            _refer(right.id).returned.statement,
          ]);
          return [
            ...left.accept(this),
            _save(
              node,
              binaryBuilder(_refer(left.id), rightCompiled.call([], {}, [])),
            ),
          ];
        } else {
          return [
            ...left.accept(this),
            ...right.accept(this),
            _save(
              node,
              _refer(left.id).property(name).call([_refer(right.id)], {}, []),
            ),
          ];
        }
      }

      if (parentModuleId == CandyType.arrayModuleId) {
        if (methodName == 'get' || methodName == 'set') {
          final array = identifier.receiver;
          final index = node.valueArguments['index'];
          final item = node.valueArguments['item'];
          final indexed = _refer(array.id).index(_refer(index.id));

          return [
            ...array.accept(this),
            ...index.accept(this),
            if (methodName == 'get')
              _save(node, indexed)
            else ...[
              ...item.accept(this),
              _save(node, indexed.assign(_refer(item.id))),
            ],
          ];
        }
      } else if (parentModuleId == CandyType.and.virtualModuleId) {
        return lazyBoolExpression('and', (l, r) => l.and(r));
      } else if (parentModuleId == CandyType.or.virtualModuleId) {
        return lazyBoolExpression('or', (l, r) => l.or(r));
      } else if (parentModuleId == CandyType.implies.virtualModuleId) {
        return lazyBoolExpression('implies', (l, r) => l.negate().or(r));
      } else if (parentModuleId == CandyType.opposite.virtualModuleId) {
        assert(methodName == 'opposite');
        final receiver = identifier.receiver;
        if (isAssignableTo(context, Tuple2(receiver.type, CandyType.bool))) {
          return [
            ...receiver.accept(this),
            _save(node, _refer(receiver.id).negate()),
          ];
        }
      } else if (parentModuleId == CandyType.comparable.virtualModuleId) {
        final relevantMethods = [
          'lessThan',
          'lessThanOrEqual',
          'greaterThan',
          'greaterThanOrEqual',
        ];
        if (relevantMethods.contains(methodName)) {
          final left = identifier.receiver;
          final right = node.valueArguments['other'];
          return [
            ...left.accept(this),
            ...right.accept(this),
            if (methodName == 'lessThan')
              _save(node, _refer(left.id).lessThan(_refer(right.id)))
            else if (methodName == 'lessThanOrEqual')
              _save(node, _refer(left.id).lessOrEqualTo(_refer(right.id)))
            else if (methodName == 'greaterThan')
              _save(node, _refer(left.id).greaterThan(_refer(right.id)))
            else
              _save(node, _refer(left.id).greaterOrEqualTo(_refer(right.id))),
          ];
        }
      } else if (parentModuleId == CandyType.equals.virtualModuleId) {
        assert(methodName == 'equals' || methodName == 'notEquals');
        final left = identifier.receiver;
        final right = node.valueArguments['other'];
        return [
          ...left.accept(this),
          ...right.accept(this),
          if (methodName == 'equals')
            _save(node, _refer(left.id).equalTo(_refer(right.id)))
          else
            _save(node, _refer(left.id).notEqualTo(_refer(right.id))),
        ];
      }
    }

    return [
      ...node.target.accept(this),
      for (final argument in node.valueArguments.values)
        ...argument.accept(this),
      _save(
        node,
        _refer(node.target.id).call(
          [
            for (final entry in node.valueArguments.entries)
              _refer(entry.value.id),
          ],
          {},
          [],
        ),
      ),
    ];
  }

  @override
  List<dart.Code> visitConstructorCallExpression(
      ConstructorCallExpression node) {
    return [
      for (final argument in node.valueArguments.values)
        ...argument.accept(this),
      _save(
        node,
        _refer(getId(node.class_.id)).call(
          [
            for (final entry in node.valueArguments.entries)
              _refer(entry.value.id),
          ],
          {},
          [],
        ),
      ),
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
  List<dart.Code> visitIfExpression(IfExpression node) {
    List<dart.Code> visitBody(List<Expression> body) => [
          for (final expression in body) ...expression.accept(this),
          if (body.isNotEmpty && node.type != CandyType.unit)
            _refer(node.id).assign(_refer(body.last.id)).statement,
        ];

    return [
      ...node.condition.accept(this),
      dart.literalNull.assignVar(_name(node.id)).statement,
      dart.Code('if (${_name(node.condition.id)}) {'),
      ...visitBody(node.thenBody),
      dart.Code('} else {'),
      ...visitBody(node.elseBody),
      dart.Code('}'),
    ];
  }

  @override
  List<dart.Code> visitLoopExpression(LoopExpression node) => [
        dart.literalNull.assignVar(_name(node.id)).statement,
        dart.Code('${_label(node.id)}:\nwhile (true) {'),
        for (final expression in node.body) ...expression.accept(this),
        dart.Code('}'),
      ];

  @override
  List<dart.Code> visitWhileExpression(WhileExpression node) => [
        dart.literalNull.assignVar(_name(node.id)).statement,
        dart.Code('${_label(node.id)}:\nwhile (true) {'),
        ...node.condition.accept(this),
        dart.Code('if (!${_name(node.condition.id)}) break;'),
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

  @override
  List<dart.Code> visitAssignmentExpression(AssignmentExpression node) => [
        ...node.right.accept(this),
        node.left.identifier
            .maybeMap(
              property: (property) => dart.refer(
                property.id.simplePath.last.nameOrNull ??
                    (throw CompilerError.internalError(
                        'Path must be path to property.')),
                declarationIdToImportUrl(context, property.id.parent),
              ),
              localProperty: (property) =>
                  _refer(getExpression(context, property.id).value.id),
              orElse: () => throw CompilerError.internalError('Left side of '
                  'assignment can only be property or local property '
                  'identifier, but was ${node.left.runtimeType} '
                  '(${node.left})'),
            )
            .assign(_refer(node.right.id))
            .statement,
      ];

  @override
  List<dart.Code> visitIsExpression(IsExpression node) {
    final instance = _refer(node.instance.id);
    final check = _compileIs(instance, node.typeToCheck);
    return [
      ...node.instance.accept(this),
      _save(node, node.isNegated ? check.parenthesized.negate() : check),
    ];
  }

  dart.Expression _compileIs(dart.Expression instance, CandyType type) {
    dart.Expression compileSimple() => instance.isA(compileType(context, type));

    return type.map(
      user: (_) => compileSimple(),
      this_: (_) => throw CompilerError.internalError(
        "`This`-type wasn't resolved before compiling it to Dart.",
      ),
      tuple: (_) => compileSimple(),
      function: (_) => compileSimple(),
      union: (type) => type.types
          .map((t) => _compileIs(instance, t))
          .reduce((value, element) => value.or(element))
          .parenthesized,
      intersection: (type) => type.types
          .map((t) => _compileIs(instance, t))
          .reduce((value, element) => value.and(element))
          .parenthesized,
      parameter: (_) => compileSimple(),
      reflection: (_) => compileSimple(),
    );
  }

  static String _name(DeclarationLocalId id) => '_${id.value}';
  static dart.Expression _refer(DeclarationLocalId id) => dart.refer(_name(id));
  dart.Code _save(
    Expression source,
    dart.Expression lowered, {
    bool isMutable = false,
  }) {
    final type = compileType(context, source.type);
    if (isMutable) {
      return lowered.assignVar(_name(source.id), type).statement;
    } else {
      return lowered.assignFinal(_name(source.id), type).statement;
    }
  }

  List<dart.Code> _saveSingle(
    Expression source,
    dart.Expression lowered, {
    bool isMutable = false,
  }) =>
      [_save(source, lowered, isMutable: isMutable)];

  String _label(DeclarationLocalId id) => '_label_${id.value}';
}

class ModuleExpression extends dart.InvokeExpression {
  ModuleExpression(QueryContext context, this.moduleId)
      : assert(context != null),
        assert(moduleId != null),
        super.constOf(
          compileType(context, CandyType.module),
          [dart.literalString(moduleId.toString())],
          {},
          [],
        );

  final ModuleId moduleId;
}

extension on dart.Expression {
  dart.Expression get parenthesized => dart.CodeExpression(dart.Block.of([
        const dart.Code('('),
        code,
        const dart.Code(')'),
      ]));
}

dart.Expression lambdaOf(List<dart.Code> code) {
  final body = dart.Block((b) => b..statements.addAll(code));
  return dart.Method((b) => b..body = body).closure;
}
