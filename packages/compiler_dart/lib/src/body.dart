import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'builtins.dart';
import 'constants.dart';
import 'declarations/declaration.dart';
import 'declarations/module.dart';
import 'type.dart';
import 'utils.dart';

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
    return Some(dart.Block((b) {
      b.statements.addAll(compiled);
      if (declarationId.isFunction) {
        final hir = getFunctionDeclarationHir(context, declarationId);
        if (hir.returnType == CandyType.unit) {
          b.statements.add(visitor.unitInstance.returned.statement);
        }
      }
    }));
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
      meta: (type, __) => _saveSingle(node, compileType(context, type)),
      reflection: (id, __) {
        if (id.isModule) {
          throw CompilerError.internalError(
            'Reflection identifiers pointing to modules are not yet supported in Dart compiler.; `$id`',
          );
        } else if (id.isProperty || id.isFunction) {
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

          final expression = dart.Method((b) => b
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
          return _saveSingle(node, expression);
        }
        throw CompilerError.internalError(
          'Invalid reflection target for Dart compiler: `$id`.',
        );
      },
      tuple: () {
        throw CompilerError.internalError(
          'Tried compiling a reference to `Tuple` directly.',
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
        final name = mangleName(id.simplePath.last.nameOrNull);

        if (receiver != null) {
          return [
            ...receiver.accept(this),
            _save(node, _refer(receiver.id).property(name)),
          ];
        }

        dart.Expression lowered;
        if ((id.isProperty || id.isFunction) && id.parent.isNotModule) {
          final parentId = () {
            if (id.parent.isTrait || id.parent.isClass) return id.parent;
            if (id.parent.isImpl) {
              final implHir = getImplDeclarationHir(context, id.parent);
              final moduleId = implHir.type.virtualModuleId;
              return moduleIdToDeclarationId(context, moduleId);
            }
            throw CompilerError.internalError(
              "Property or function's parent is not a module, trait, impl or class: `$id`.",
            );
          }();

          lowered = compileTypeName(context, parentId).property(name);
        } else {
          var name = id.simplePath.last.nameOrNull;
          if (name == 'assert') name = 'assert_';
          lowered = dart.refer(
            name,
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
      boolean: (value) =>
          _saveSingle(node, dart.literalBool(value).wrapInCandyBool(context)),
      integer: (value) =>
          _saveSingle(node, dart.literalNum(value).wrapInCandyInt(context)),
      string: (parts) {
        if (parts.isEmpty) {
          return _saveSingle(
            node,
            dart.literalString('').wrapInCandyString(context),
          );
        }

        String escapeForStringLiteral(String value) {
          // `code_builder` escapes single quotes and newlines, but misses the
          // following:
          return value
              .replaceAll('\\', '\\\\')
              .replaceAll('\t', '\\t')
              .replaceAll('\r', '\\r')
              .replaceAll('\$', '\\\$');
        }

        if (parts.length == 1 && parts.single is LiteralStringLiteralPart) {
          final part = parts.single as LiteralStringLiteralPart;
          return _saveSingle(
            node,
            dart
                .literalString(escapeForStringLiteral(part.value))
                .wrapInCandyString(context),
          );
        }

        final lowered = <dart.Code>[];
        for (final part in parts.whereType<InterpolatedStringLiteralPart>()) {
          lowered.addAll(part.value.accept(this));
        }

        final content = parts
            .map((p) => p.when(
                  literal: escapeForStringLiteral,
                  interpolated: (expression) => '\${${_name(expression.id)}}',
                ))
            .join();
        lowered.add(
          _save(node, dart.literalString(content).wrapInCandyString(context)),
        );

        return lowered;
      },
      lambda: (parameters, expressions, returnType, receiverType) {
        final closure = dart.Method((b) {
          if (receiverType != null) {
            b.requiredParameters
                .add(dart.Parameter((b) => b..name = _lambdaThisName(node)));
          }

          final params = parameters.map((p) => dart.Parameter((b) {
                final parserSeparatedById = DeclarationId(
                  ResourceId(
                    PackageId('petit_parser'),
                    'src/parsers/module.candy',
                  ),
                )
                    .inner(DeclarationPathData.trait('Parser'))
                    .inner(DeclarationPathData.function('separatedBy'));
                final iterableUnsafeEquals = DeclarationId(
                  ResourceId(PackageId.core, 'src/collections/iterable.candy'),
                )
                    .inner(DeclarationPathData.trait('Iterable'))
                    .inner(DeclarationPathData.function('unsafeEquals'));
                final exceptionIds = [
                  DeclarationLocalId(parserSeparatedById, 20),
                  DeclarationLocalId(parserSeparatedById, 32),
                  DeclarationLocalId(iterableUnsafeEquals, 14),
                ];
                if (!exceptionIds.contains(node.id)) {
                  b.type = compileType(context, p.type);
                }
                b.name = p.name;
              }));
          b.requiredParameters.addAll(params);

          final loweredExpressions = expressions.expand((e) => e.accept(this));
          b.body = dart.Block((b) {
            b.statements.addAll(loweredExpressions);
            if (returnType == CandyType.unit) {
              b.statements.add(unitInstance.returned.statement);
            }
          });
        }).closure;

        dart.Reference type;
        final linkedHashMapRemoveId = DeclarationLocalId(
          DeclarationId(
            ResourceId(
              PackageId.core,
              'src/collections/map/linked_hash_map.candy',
            ),
          )
              .inner(DeclarationPathData.impl('LinkedHashMap'), 1)
              .inner(DeclarationPathData.function('remove')),
          4,
        );
        if (node.id == linkedHashMapRemoveId) {
          type = dart.FunctionType((b) => b
            ..requiredParameters.add(dart.refer('MapEntry<Key, Value>'))
            ..returnType = compileType(context, CandyType.bool));
        }
        return [
          _save(node, closure, type: type),
        ];
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
  List<dart.Code> visitFunctionCallExpression(FunctionCallExpression node) {
    final surroundingDeclarationName = declarationId.simplePath.last.nameOrNull;
    return [
      ...node.target.accept(this),
      for (final argument in node.valueArguments.values)
        ...argument.accept(this),
      if (surroundingDeclarationName == 'entryForKey' && node.id.value == 10)
        _save(
          node,
          _refer(node.target.id)
              .call(
                node.valueArguments.entries.map((it) => _refer(it.value.id)),
              )
              .asA(compileType(context, CandyType.bool)),
        )
      else
        _save(
          node,
          _refer(node.target.id).call(
            [
              if (surroundingDeclarationName == 'entryForKey' &&
                  node.id.value == 10)
                _refer(node.valueArguments.values.single.id)
                    .asA(dart.refer('dynamic', dartCoreUrl))
              else
                // if (surroundingDeclarationName == 'set' && node.id.value == 17)
                //   _refer(node.valueArguments.values.single.id)
                //       .asA(dart.refer('dynamic', dartCoreUrl))
                // else
                for (final entry in node.valueArguments.entries)
                  _refer(entry.value.id),
            ],
            {},
            node.typeArguments.map((it) => compileType(context, it)).toList(),
          ),
        ),
    ];
  }

  @override
  List<dart.Code> visitConstructorCallExpression(
    ConstructorCallExpression node,
  ) {
    return [
      for (final argument in node.valueArguments.values)
        ...argument.accept(this),
      _save(
        node,
        compileTypeName(context, node.class_.id).call(
          [
            for (final entry in node.valueArguments.entries)
              _refer(entry.value.id),
          ],
          {},
          node.typeArguments.map((it) => compileType(context, it)).toList(),
        ),
      ),
    ];
  }

  @override
  List<dart.Code> visitExpressionCallExpression(
          ExpressionCallExpression node) =>
      [
        ...node.target.accept(this),
        for (final argument in node.valueArguments) ...argument.accept(this),
        _save(
          node,
          _refer(node.target.id).call(
            [for (final value in node.valueArguments) _refer(value.id)],
            {},
            [],
          ),
        ),
      ];

  @override
  List<dart.Code> visitReturnExpression(ReturnExpression node) => [
        // TODO(JonasWanke): support labeled returns
        if (node.expression != null) ...[
          ...node.expression.accept(this),
          _refer(node.expression.id).returned.statement,
        ] else
          dart.Code('return;'),
        _save(node, unitInstance),
      ];

  @override
  List<dart.Code> visitIfExpression(IfExpression node) {
    List<dart.Code> visitBody(List<Expression> body) => [
          for (final expression in body) ...expression.accept(this),
          _refer(node.id)
              .assign(body.isNotEmpty ? _refer(body.last.id) : unitInstance)
              .statement,
        ];

    return [
      ...node.condition.accept(this),
      dart.literalNull
          .assignVarTypesafe(_name(node.id), compileType(context, node.type)),
      dart.Code('if (${_name(node.condition.id)}.value) {'),
      ...visitBody(node.thenBody),
      dart.Code('} else {'),
      ...visitBody(node.elseBody),
      dart.Code('}'),
    ];
  }

  @override
  List<dart.Code> visitLoopExpression(LoopExpression node) => [
        dart.literalNull
            .assignVarTypesafe(_name(node.id), compileType(context, node.type)),
        dart.Code('${_label(node.id)}:\nwhile (true) {'),
        for (final expression in node.body) ...expression.accept(this),
        dart.Code('}'),
      ];

  @override
  List<dart.Code> visitWhileExpression(WhileExpression node) => [
        unitInstance.assignVarTypesafe(
            _name(node.id), compileType(context, node.type)),
        dart.Code('${_label(node.id)}:\nwhile (true) {'),
        ...node.condition.accept(this),
        dart.Code('if (!${_name(node.condition.id)}.value) {'),
        _refer(node.id).assign(unitInstance).statement,
        dart.Code('break;'),
        dart.Code('}'),
        for (final expression in node.body) ...expression.accept(this),
        dart.Code('}'),
      ];

  @override
  List<dart.Code> visitForExpression(ForExpression node) {
    final iteratorName = '${_name(node.id)}_iterator';
    final rawItemName = '${_name(node.id)}_rawItem';
    return [
      unitInstance.assignVarTypesafe(
          _name(node.id), compileType(context, node.type)),
      ...node.iterable.accept(this),
      _refer(node.iterable.id)
          .property('iterator')
          .call([], {}, [])
          .assignFinal(iteratorName)
          .statement,
      dart.Code('${_label(node.id)}:\nwhile (true) {'),
      dart
          .refer(iteratorName)
          .property('next')
          .call([], {}, [])
          .assignFinal(rawItemName)
          .statement,
      dart.Code('if ('),
      dart
          .refer(rawItemName)
          .isA(
            dart.refer(
              'None',
              moduleIdToImportUrl(context, ModuleId.coreMaybe),
            ),
          )
          .code,
      dart.Code(') {'),
      _refer(node.id).assign(unitInstance).statement,
      dart.Code('break;'),
      dart.Code('}'),
      dart
          .refer(rawItemName)
          .asA(compileType(context, CandyType.some(node.itemType)))
          .property('value')
          .assignFinal(node.variableName)
          .statement,
      for (final expression in node.body) ...expression.accept(this),
      dart.Code('}'),
    ];
  }

  @override
  List<dart.Code> visitBreakExpression(BreakExpression node) => [
        if (node.expression != null) ...[
          ...node.expression.accept(this),
          _refer(node.scopeId).assign(_refer(node.expression.id)).statement,
        ],
        dart.Code('break ${_label(node.scopeId)};'),
        _save(node, unitInstance),
      ];

  @override
  List<dart.Code> visitContinueExpression(ContinueExpression node) => [
        dart.Code('continue ${_label(node.scopeId)};'),
        _save(node, unitInstance),
      ];

  @override
  List<dart.Code> visitThrowExpression(ThrowExpression node) {
    return [
      ...node.error.accept(this),
      _save(node, _refer(node.error.id).thrown),
    ];
  }

  @override
  List<dart.Code> visitAssignmentExpression(AssignmentExpression node) {
    final code = [
      ...node.right.accept(this),
    ];
    final left = node.left.identifier.maybeMap(
      property: (property) {
        final name = property.id.simplePath.last.nameOrNull ??
            (throw CompilerError.internalError(
                'Path must be path to property.'));
        if (property.receiver != null) {
          code.addAll(property.receiver.accept(this));
          return _refer(property.receiver.id).property(name);
        }

        final parent = property.id.parent;
        if (parent.isModule) {
          return dart.refer(name, declarationIdToImportUrl(context, parent));
        } else {
          assert(getPropertyDeclarationHir(context, property.id).isStatic);
          return compileTypeName(context, parent).property(name);
        }
      },
      localProperty: (property) =>
          _refer(getExpression(context, property.id).value.id),
      orElse: () => throw CompilerError.internalError('Left side of '
          'assignment can only be property or local property '
          'identifier, but was ${node.left.runtimeType} '
          '(${node.left})'),
    );

    code.add(left.assign(_refer(node.right.id)).statement);
    code.add(_save(node, left));
    return code;
  }

  @override
  List<dart.Code> visitAsExpression(AsExpression node) {
    final instance = _refer(node.instance.id);
    final type = _compileAsType(node.typeToCheck);
    return [
      ...node.instance.accept(this),
      _save(node, instance.asA(type)),
    ];
  }

  dart.Expression _compileAsType(CandyType type) {
    dart.Expression compileSimple() => compileType(context, type);

    return type.map(
      user: (_) => compileSimple(),
      this_: (_) => throw CompilerError.internalError(
        "`This`-type wasn't resolved before compiling it to Dart.",
      ),
      tuple: (_) => compileSimple(),
      function: (_) => compileSimple(),
      union: (type) => dart.refer('dynamic', dartCoreUrl),
      intersection: (type) => dart.refer('dynamic', dartCoreUrl),
      parameter: (_) => compileSimple(),
      meta: (_) => compileSimple(),
      reflection: (_) => compileSimple(),
    );
  }

  @override
  List<dart.Code> visitIsExpression(IsExpression node) {
    final instance = _refer(node.instance.id);
    final check = _compileIs(instance, node.typeToCheck);
    return [
      ...node.instance.accept(this),
      _save(
        node,
        (node.isNegated ? check.parenthesized.negate() : check)
            .wrapInCandyBool(context),
      ),
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
      meta: (_) => compileSimple(),
      reflection: (_) => compileSimple(),
    );
  }

  @override
  List<dart.Code> visitTupleExpression(TupleExpression node) {
    return [
      for (final argument in node.arguments) ...argument.accept(this),
      _save(
        node,
        compileType(context, node.type).call(
          node.arguments.map((it) => _refer(it.id)).toList(),
          {},
          [],
        ),
      ),
    ];
  }

  dart.Expression get unitInstance {
    final moduleId = CandyType.unit.virtualModuleId;
    final declarationId = moduleIdToDeclarationId(context, moduleId);
    return compileTypeName(context, declarationId).call([], {}, []);
  }

  static String _name(DeclarationLocalId id) => '_${id.value}';
  static dart.Expression _refer(DeclarationLocalId id) => dart.refer(_name(id));
  dart.Code _save(
    Expression source,
    dart.Expression lowered, {
    bool isMutable = false,
    dart.Reference type,
  }) {
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

  dart.Code assignVarTypesafe(String name, dart.Reference type) {
    return dart.CodeExpression(dart.Block.of([
      type.code,
      const dart.Code(' '),
      dart.refer(name).code,
      const dart.Code('='),
      code,
    ])).statement;
  }
}

dart.Expression lambdaOf(List<dart.Code> code) {
  final body = dart.Block((b) => b..statements.addAll(code));
  return dart.Method((b) => b..body = body).closure;
}
