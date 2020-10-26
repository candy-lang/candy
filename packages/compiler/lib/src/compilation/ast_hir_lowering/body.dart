import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;
import 'package:parser/parser.dart' show SourceSpan;

import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast.dart';
import '../hir.dart' as hir;
import '../hir/ids.dart';
import 'declarations/declarations.dart';
import 'declarations/function.dart';
import 'declarations/module.dart';
import 'declarations/property.dart';
import 'general.dart';
import 'type.dart';

final getExpression = Query<DeclarationLocalId, Option<hir.Expression>>(
  'getExpression',
  provider: (context, id) {
    final body = getBody(context, id.declarationId);
    if (body is None) return None();

    final visitor = IdFinderVisitor(id);
    for (final expression in body.value) {
      final result = expression.accept(visitor);
      if (result is Some) return result;
    }
    return None();
  },
);

class IdFinderVisitor extends hir.ExpressionVisitor<Option<hir.Expression>> {
  const IdFinderVisitor(this.id) : assert(id != null);

  final DeclarationLocalId id;

  @override
  Option<hir.Expression> visitIdentifierExpression(IdentifierExpression node) {
    if (node.id == id) return Some(node);
    if (node.identifier is hir.PropertyIdentifier) {
      final target = (node.identifier as hir.PropertyIdentifier).target;
      if (target != null) return target.accept(this);
    }
    return None();
  }

  @override
  Option<hir.Expression> visitLiteralExpression(LiteralExpression node) {
    if (node.id == id) return Some(node);
    if (node.literal is hir.StringLiteral) {
      final literal = node.literal as hir.StringLiteral;
      for (final part in literal.parts) {
        if (part is hir.InterpolatedStringLiteralPart) {
          final result = part.value.accept(this);
          if (result is Some) return result;
        }
      }
    }
    if (node.literal is hir.LambdaLiteral) {
      final literal = node.literal as hir.LambdaLiteral;
      for (final expression in literal.expressions) {
        final result = expression.accept(this);
        if (result is Some) return result;
      }
    }
    return None();
  }

  @override
  Option<Expression> visitPropertyExpression(PropertyExpression node) {
    if (node.id == id) return Some(node);
    return node.initializer.accept(this);
  }

  @override
  Option<hir.Expression> visitNavigationExpression(NavigationExpression node) {
    if (node.id == id) return Some(node);
    return node.target.accept(this);
  }

  @override
  Option<hir.Expression> visitCallExpression(CallExpression node) {
    if (node.id == id) return Some(node);
    for (final argument in node.valueArguments) {
      final result = argument.accept(this);
      if (result is Some) return result;
    }
    return node.target.accept(this);
  }

  @override
  Option<hir.Expression> visitFunctionCallExpression(
    FunctionCallExpression node,
  ) {
    if (node.id == id) return Some(node);
    for (final argument in node.valueArguments.values) {
      final result = argument.accept(this);
      if (result is Some) return result;
    }
    return node.target.accept(this);
  }

  @override
  Option<hir.Expression> visitReturnExpression(ReturnExpression node) {
    if (node.id == id) return Some(node);
    if (node.expression != null) return node.expression.accept(this);
    return None();
  }

  @override
  Option<hir.Expression> visitLoopExpression(LoopExpression node) {
    if (node.id == id) return Some(node);
    for (final expression in node.body) {
      final result = expression.accept(this);
      if (result is Some) return result;
    }
    return None();
  }

  @override
  Option<hir.Expression> visitBreakExpression(BreakExpression node) {
    if (node.id == id) return Some(node);
    if (node.expression != null) return node.expression.accept(this);
    return None();
  }

  @override
  Option<hir.Expression> visitContinueExpression(ContinueExpression node) {
    if (node.id == id) return Some(node);
    return None();
  }
}

final getBody = Query<DeclarationId, Option<List<hir.Expression>>>(
  'getBody',
  provider: (context, declarationId) =>
      lowerBodyAstToHir(context, declarationId).mapValue((v) => v.first),
);
final getBodyAstToHirIds = Query<DeclarationId, Option<BodyAstToHirIds>>(
  'getBodyAstToHirIds',
  provider: (context, declarationId) =>
      lowerBodyAstToHir(context, declarationId).mapValue((v) => v.second),
);
final Query<DeclarationId,
        Option<Tuple2<List<hir.Expression>, BodyAstToHirIds>>>
    lowerBodyAstToHir =
    Query<DeclarationId, Option<Tuple2<List<hir.Expression>, BodyAstToHirIds>>>(
  'lowerBodyAstToHir',
  provider: (context, declarationId) {
    if (declarationId.isFunction) {
      final functionAst = getFunctionDeclarationAst(context, declarationId);
      if (functionAst.body == null) return None();

      final result = FunctionContext.lowerFunction(context, declarationId);
      // ignore: only_throw_errors, Iterables of errors are also handled.
      if (result is Error) throw result.error;
      return Some(result.value);
    } else if (declarationId.isProperty) {
      final propertyAst = getPropertyDeclarationAst(context, declarationId);
      if (propertyAst.initializer == null) return None();

      final result = PropertyContext.lowerProperty(context, declarationId);
      // ignore: only_throw_errors, Iterables of errors are also handled.
      if (result is Error) throw result.error;
      return Some(Tuple2([result.value.first], result.value.second));
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported body.',
        location: ErrorLocation(
          declarationId.resourceId,
          getDeclarationAst(context, declarationId).span,
        ),
      );
    }
  },
);

abstract class Context {
  QueryContext get queryContext;
  DeclarationId get declarationId;
  ModuleId get moduleId => declarationIdToModuleId(queryContext, declarationId);
  ResourceId get resourceId => declarationId.resourceId;

  Option<Context> get parent;

  Option<hir.CandyType> get expressionType;
  bool isValidExpressionType(hir.CandyType type) {
    return expressionType.when(
      some: (expressionType) =>
          isAssignableTo(queryContext, Tuple2(type, expressionType)),
      none: () => true,
    );
  }

  DeclarationLocalId getId([
    dynamic /* ast.Expression | ast.ValueParameter */ expressionOrParameter,
  ]);
  BodyAstToHirIds get idMap;

  List<hir.Identifier> resolveIdentifier(String name);
  void addIdentifier(hir.LocalPropertyIdentifier identifier);

  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveReturn(
    Option<String> label,
  );

  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveBreak(
    Option<String> label,
  );
  Option<DeclarationLocalId> resolveContinue(Option<String> label) =>
      resolveBreak(label).mapValue((values) => values.first);

  Result<List<hir.Expression>, List<ReportedCompilerError>> lower(
    ast.Expression expression,
  ) {
    Result<List<hir.Expression>, List<ReportedCompilerError>> result;
    if (expression is ast.Literal) {
      result = lowerLiteral(expression);
    } else if (expression is ast.StringLiteral) {
      result = lowerStringLiteral(expression);
    } else if (expression is ast.LambdaLiteral) {
      result = lowerLambdaLiteral(expression);
    } else if (expression is ast.Identifier) {
      result = lowerIdentifier(expression);
    } else if (expression is ast.PropertyDeclaration) {
      result = lowerProperty(expression);
    } else if (expression is ast.CallExpression) {
      result = lowerCall(expression);
    } else if (expression is ast.ReturnExpression) {
      result = lowerReturn(expression);
    } else if (expression is ast.LoopExpression) {
      result = lowerLoop(expression);
    } else if (expression is ast.BreakExpression) {
      result = lowerBreak(expression);
    } else if (expression is ast.ContinueExpression) {
      result = lowerContinue(expression);
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported expression: $expression (`${expression.runtimeType}`).',
        location: ErrorLocation(resourceId, expression.span),
      );
    }

    assert(result != null);
    assert(result is Error ||
        result.value.isNotEmpty &&
            result.value.every((r) => isValidExpressionType(r.type)));
    assert(result is Ok || result.error.isNotEmpty);
    return result;
  }

  Result<hir.Expression, List<ReportedCompilerError>> lowerUnambiguous(
    ast.Expression expression,
  ) {
    final result = lower(expression);
    if (result is Error) return Error(result.error);

    if (result.value.isEmpty) {
      assert(expressionType is Some);
      return Error([
        CompilerError.invalidExpressionType(
          'Expression could not be resolved to match type `${expressionType.value}`.',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    } else if (result.value.length > 1) {
      return Error([
        CompilerError.ambiguousExpression(
          'Expression is ambiguous.',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }
    return Ok(result.value.single);
  }
}

extension<T, E> on Iterable<Result<T, List<E>>> {
  Result<List<T>, List<E>> merge() {
    final errors = whereType<Error<T, List<E>>>();
    if (errors.isNotEmpty) return Error(errors.expand((e) => e.error).toList());

    final oks = whereType<Ok<T, List<E>>>();
    return Ok(oks.map((ok) => ok.value).toList());
  }
}

extension<T, E> on Iterable<Result<List<T>, List<E>>> {
  Result<List<T>, List<E>> merge() {
    final errors = whereType<Error<List<T>, List<E>>>();
    if (errors.isNotEmpty) return Error(errors.expand((e) => e.error).toList());

    final oks = whereType<Ok<List<T>, List<E>>>();
    return Ok(oks.expand((ok) => ok.value).toList());
  }
}

abstract class InnerContext extends Context {
  InnerContext(Context parent)
      : assert(parent != null),
        parent = Some(parent);

  @override
  QueryContext get queryContext => parent.value.queryContext;
  @override
  DeclarationId get declarationId => parent.value.declarationId;

  @override
  final Option<Context> parent;

  @override
  Option<hir.CandyType> get expressionType => parent.value.expressionType;

  @override
  DeclarationLocalId getId([
    dynamic /* ast.Expression | ast.ValueParameter */ expressionOrParameter,
  ]) =>
      parent.value.getId(expressionOrParameter);
  @override
  BodyAstToHirIds get idMap => parent.value.idMap;

  @override
  List<hir.Identifier> resolveIdentifier(String name) =>
      parent.value.resolveIdentifier(name);
  @override
  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveReturn(
    Option<String> label,
  ) =>
      parent.value.resolveReturn(label);

  @override
  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveBreak(
    Option<String> label,
  ) =>
      parent.value.resolveBreak(label);
}

class ContextContext extends Context {
  ContextContext(this.queryContext, this.declarationId)
      : assert(queryContext != null),
        assert(declarationId != null);

  @override
  final QueryContext queryContext;
  @override
  final DeclarationId declarationId;

  @override
  Option<Context> get parent => None();
  @override
  Option<hir.CandyType> get expressionType => None();

  var _nextId = 0;
  var _idMap = BodyAstToHirIds();
  @override
  BodyAstToHirIds get idMap => _idMap;
  @override
  DeclarationLocalId getId([
    dynamic /* ast.Expression | ast.ValueParameter */ expressionOrParameter,
  ]) {
    int astId;
    if (expressionOrParameter is ast.Expression) {
      astId = expressionOrParameter.id;
    } else if (expressionOrParameter is ast.ValueParameter) {
      astId = expressionOrParameter.id;
    } else if (expressionOrParameter != null) {
      throw CompilerError.internalError(
        '`ContextContext.getId()` called with an invalid `expressionOrParameter` argument: `$expressionOrParameter`.',
      );
    }

    final existing = _idMap.map[astId];
    if (existing != null) return existing;

    final id = DeclarationLocalId(declarationId, _nextId++);
    if (expressionOrParameter == null) return id;

    _idMap = _idMap.withMapping(astId, id);
    return id;
  }

  @override
  List<hir.Identifier> resolveIdentifier(String name) {
    // resolve `this`
    if (name == 'this') {
      if (declarationId.isConstructor) {
        return [];
      } else if (declarationId.isFunction) {
        final function = getFunctionDeclarationHir(queryContext, declarationId);
        if (function.isStatic) return [];
      } else if (declarationId.isProperty) {
        final function = getPropertyDeclarationHir(queryContext, declarationId);
        if (function.isStatic) return [];
      } else {
        throw CompilerError.internalError(
          '`ContextContext` is not within a constructor, function or property: `$declarationId`.',
        );
      }

      if (!declarationId.hasParent) return [];
      final parent = declarationId.parent;
      if (parent.isTrait || parent.isImpl || parent.isClass) {
        return [hir.Identifier.this_()];
      }
      return [];
    }

    // resolve `field` in a getter/setter
    // TODO(JonasWanke): resolve `field` in property accessors

    // TODO: check whether properties/functions are static or not and whether we have an instance

    hir.Identifier convertDeclarationId(DeclarationId id) {
      hir.CandyType type;
      if (id.isFunction) {
        final functionHir = getFunctionDeclarationHir(queryContext, id);
        type = hir.CandyType.function(
          parameterTypes: [
            for (final parameter in functionHir.parameters) parameter.type,
          ],
          returnType: functionHir.returnType,
        );
      } else if (id.isProperty) {
        type = getPropertyDeclarationHir(queryContext, id).type;
      } else {
        throw CompilerError.unsupportedFeature(
          "Matched identifier `$name`, but it's neither a function nor a property.",
        );
      }
      return hir.Identifier.property(id, type);
    }

    // search the current file (from the curent module to the root)
    assert(declarationId.hasParent);
    var moduleId = declarationIdToModuleId(queryContext, declarationId.parent);
    while (true) {
      final declarationId = moduleIdToDeclarationId(queryContext, moduleId);
      List<DeclarationId> innerIds;
      if (declarationId.isModule) {
        innerIds =
            getModuleDeclarationHir(queryContext, moduleId).innerDeclarationIds;
      } else if (declarationId.isTrait) {
        innerIds = getTraitDeclarationHir(queryContext, declarationId)
            .innerDeclarationIds;
      } else if (declarationId.isImpl) {
        innerIds = getImplDeclarationHir(queryContext, declarationId)
            .innerDeclarationIds;
      } else if (declarationId.isClass) {
        innerIds = getClassDeclarationHir(queryContext, declarationId)
            .innerDeclarationIds;
      } else {
        throw CompilerError.internalError(
          'Lowered a body whose declaration was not inside a module, trait, impl or class.',
          location: ErrorLocation(resourceId),
        );
      }

      final matches = innerIds
          .where((id) => id.simplePath.last.nameOrNull == name)
          .map(convertDeclarationId);
      if (matches.isNotEmpty) return matches.toList();

      if (moduleId.hasNoParent) break;
      moduleId = moduleId.parent;
      final newDeclarationId =
          moduleIdToOptionalDeclarationId(queryContext, moduleId);
      if (newDeclarationId is None ||
          newDeclarationId.value.resourceId != resourceId) {
        break;
      }
    }

    // search use-lines
    return findIdentifierInUseLines(
      queryContext,
      Tuple4(resourceId, name, false, false),
    ).map(convertDeclarationId).toList();
  }

  @override
  void addIdentifier(hir.LocalPropertyIdentifier identifier) {
    throw CompilerError.internalError(
      "Can't add an identifier to a `ContextContext`.",
    );
  }

  @override
  Option<Tuple2<DeclarationLocalId, Option<CandyType>>> resolveReturn(
    Option<String> label,
  ) =>
      None();
  @override
  Option<Tuple2<DeclarationLocalId, Option<CandyType>>> resolveBreak(
    Option<String> label,
  ) =>
      None();
}

class FunctionContext extends InnerContext {
  factory FunctionContext._create(QueryContext queryContext, DeclarationId id) {
    final parent = ContextContext(queryContext, id);
    final ast = getFunctionDeclarationAst(queryContext, id);
    final identifiers = {
      for (final parameter in ast.valueParameters)
        parameter.name.name: hir.Identifier.parameter(
          parent.getId(parameter),
          parameter.name.name,
          astTypeToHirType(
            parent.queryContext,
            Tuple2(
              declarationIdToModuleId(
                parent.queryContext,
                parent.declarationId,
              ),
              parameter.type,
            ),
          ),
        ),
    };

    return FunctionContext._(
      parent,
      identifiers,
      getFunctionDeclarationHir(queryContext, id).returnType,
      ast.body,
    );
  }
  FunctionContext._(
    Context parent,
    this._identifiers,
    this.returnType,
    this.body,
  )   : assert(_identifiers != null),
        assert(returnType != null),
        assert(body != null),
        super(parent);

  static Result<Tuple2<List<hir.Expression>, BodyAstToHirIds>,
      List<ReportedCompilerError>> lowerFunction(
    QueryContext queryContext,
    DeclarationId id,
  ) =>
      FunctionContext._create(queryContext, id)._lowerBody();

  final Map<String, hir.Identifier> _identifiers;
  final hir.CandyType returnType;
  final ast.LambdaLiteral body;

  @override
  void addIdentifier(hir.LocalPropertyIdentifier identifier) {
    _identifiers[identifier.name] = identifier;
  }

  @override
  List<Identifier> resolveIdentifier(String name) {
    final result = _identifiers[name];
    if (result != null) return [result];
    return parent.value.resolveIdentifier(name);
  }

  @override
  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveReturn(
    Option<String> label,
  ) {
    if (label is None ||
        label == Some(declarationId.simplePath.last.nameOrNull)) {
      return Some(Tuple2(getId(body), Some(returnType)));
    }
    return None();
  }

  Result<Tuple2<List<hir.Expression>, BodyAstToHirIds>,
      List<ReportedCompilerError>> _lowerBody() {
    final returnsUnit = returnType == hir.CandyType.unit;

    if (!returnsUnit && body.expressions.isEmpty) {
      return Error([
        CompilerError.missingReturn(
          "Function has a return type (different than `Unit`) but doesn't contain any expressions.",
          location: ErrorLocation(
            resourceId,
            getFunctionDeclarationAst(queryContext, declarationId)
                .representativeSpan,
          ),
        ),
      ]);
    }

    final results = <Result<hir.Expression, List<ReportedCompilerError>>>[];

    for (final expression in body.expressions.dropLast(returnsUnit ? 0 : 1)) {
      final lowered = innerExpressionContext(forwardsIdentifiers: true)
          .lowerUnambiguous(expression);
      results.add(lowered);
    }

    if (!returnsUnit) {
      final lowered = innerExpressionContext(expressionType: Some(returnType))
          .lowerUnambiguous(body.expressions.last);
      if (lowered is Error) {
        results.add(lowered);
      } else if (lowered.value is hir.ReturnExpression) {
        results.add(lowered);
      } else {
        results.add(Ok(
          hir.Expression.return_(getId(), getId(body), lowered.value),
        ));
      }
    }
    return results
        .merge()
        .mapValue((expressions) => Tuple2(expressions, idMap));
  }
}

class PropertyContext extends InnerContext {
  factory PropertyContext._create(QueryContext queryContext, DeclarationId id) {
    final parent = ContextContext(queryContext, id);
    final ast = getPropertyDeclarationAst(queryContext, id);

    final type = Option.of(ast.type).mapValue(
        (t) => astTypeToHirType(queryContext, Tuple2(parent.moduleId, t)));

    return PropertyContext._(parent, type, ast.initializer);
  }
  PropertyContext._(
    Context parent,
    this.type,
    this.initializer,
  )   : assert(type != null),
        assert(initializer != null),
        super(parent);

  static Result<Tuple2<hir.Expression, BodyAstToHirIds>,
      List<ReportedCompilerError>> lowerProperty(
    QueryContext queryContext,
    DeclarationId id,
  ) =>
      PropertyContext._create(queryContext, id)._lowerInitializer();

  final Option<hir.CandyType> type;
  final ast.Expression initializer;

  @override
  void addIdentifier(LocalPropertyIdentifier identifier) {}

  Result<Tuple2<hir.Expression, BodyAstToHirIds>, List<ReportedCompilerError>>
      _lowerInitializer() {
    final lowered = innerExpressionContext(expressionType: type)
        .lowerUnambiguous(initializer);

    return lowered.mapValue((e) => Tuple2(e, idMap));
  }
}

class LambdaContext extends InnerContext {
  LambdaContext(
    Context parent,
    this.id,
    this.label,
    this._identifiers,
  )   : assert(id != null),
        assert(label != null),
        assert(_identifiers != null),
        super(parent);

  final DeclarationLocalId id;
  final Option<String> label;
  final Map<String, hir.Identifier> _identifiers;

  @override
  void addIdentifier(hir.LocalPropertyIdentifier identifier) {
    _identifiers[identifier.name] = identifier;
  }

  @override
  List<Identifier> resolveIdentifier(String name) {
    final result = _identifiers[name];
    if (result != null) return [result];
    return parent.value.resolveIdentifier(name);
  }

  @override
  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveReturn(
    Option<String> label,
  ) {
    if (label is None || label == this.label) {
      final returnType = parent.value.expressionType
          .mapValue((type) => (type as hir.FunctionCandyType).returnType);

      return Some(Tuple2(id, returnType));
    }
    return parent.flatMapValue((context) => context.resolveReturn(label));
  }
}

class ReturnExpressionVisitor extends DoNothingExpressionVisitor {
  final returnTypes = <hir.CandyType>{};

  @override
  void visitReturnExpression(ReturnExpression node) {
    returnTypes.add(node.expression.type);
  }
}

class ExpressionContext extends InnerContext {
  ExpressionContext(
    Context parent, {
    this.expressionType = const None(),
    this.forwardsIdentifiers = false,
  })  : assert(expressionType != null),
        assert(forwardsIdentifiers != null),
        super(parent);

  @override
  final Option<hir.CandyType> expressionType;

  final bool forwardsIdentifiers;

  @override
  void addIdentifier(LocalPropertyIdentifier identifier) {
    if (!forwardsIdentifiers) return;

    parent.value.addIdentifier(identifier);
  }
}

class LoopContext extends InnerContext {
  LoopContext(
    Context parent,
    this.id,
    this.label,
  )   : assert(id != null),
        assert(label != null),
        super(parent);

  final DeclarationLocalId id;
  final Option<String> label;
  final _identifiers = <String, hir.Identifier>{};

  @override
  void addIdentifier(hir.LocalPropertyIdentifier identifier) {
    _identifiers[identifier.name] = identifier;
  }

  @override
  List<Identifier> resolveIdentifier(String name) {
    final result = _identifiers[name];
    if (result != null) return [result];
    return parent.value.resolveIdentifier(name);
  }

  @override
  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveBreak(
    Option<String> label,
  ) {
    if (label is None ||
        label == this.label ||
        this.label is None && label == Some('loop')) {
      return Some(Tuple2(id, parent.value.expressionType));
    }
    return parent.flatMapValue((context) => context.resolveBreak(label));
  }
}

class BreakExpressionVisitor extends DoNothingExpressionVisitor {
  final breakTypes = <hir.CandyType>{};

  @override
  void visitBreakExpression(BreakExpression node) {
    breakTypes.add(node.expression?.type ?? hir.CandyType.unit);
  }
}

extension on Context {
  ExpressionContext innerExpressionContext({
    Option<hir.CandyType> expressionType = const None(),
    bool forwardsIdentifiers = false,
  }) {
    return ExpressionContext(
      this,
      expressionType: expressionType,
      forwardsIdentifiers: forwardsIdentifiers,
    );
  }
}

extension on Context {
  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerLiteral(
    ast.Literal<dynamic> expression,
  ) {
    final token = expression.value;
    hir.Literal literal;
    if (token is ast.BoolLiteralToken) {
      if (!isValidExpressionType(hir.CandyType.bool)) {
        return Error([
          CompilerError.invalidExpressionType(
            'Expected type `${expressionType.value}`, got `Bool`',
            location: ErrorLocation(resourceId, expression.span),
          ),
        ]);
      }
      literal = hir.Literal.boolean(token.value);
    } else if (token is ast.IntLiteralToken) {
      if (!isValidExpressionType(hir.CandyType.int)) {
        return Error([
          CompilerError.invalidExpressionType(
            'Expected type `${expressionType.value}`, got `Int`',
            location: ErrorLocation(resourceId, expression.span),
          ),
        ]);
      }
      literal = hir.Literal.integer(token.value);
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported literal.',
        location: ErrorLocation(resourceId, token.span),
      );
    }
    return Ok([
      hir.Expression.literal(getId(expression), literal),
    ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerStringLiteral(
    ast.StringLiteral expression,
  ) {
    final parts = expression.parts
        .map<Result<List<hir.StringLiteralPart>, List<ReportedCompilerError>>>(
            (part) {
      if (part is ast.LiteralStringLiteralPart) {
        return Ok([hir.StringLiteralPart.literal(part.value.value)]);
      } else if (part is ast.InterpolatedStringLiteralPart) {
        return innerExpressionContext()
            .lowerUnambiguous(part.expression)
            .mapValue((expression) =>
                [hir.StringLiteralPart.interpolated(expression)]);
      } else {
        throw CompilerError.unsupportedFeature(
          'Unsupported String literal part.',
          location: ErrorLocation(resourceId, part.span),
        );
      }
    });
    return parts.merge().mapValue((parts) => [
          hir.Expression.literal(getId(expression), hir.StringLiteral(parts)),
        ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerLambdaLiteral(
    ast.LambdaLiteral expression,
  ) {
    final type = expressionType;
    if (type is Some && type.value is! hir.FunctionCandyType) {
      return Error([
        CompilerError.invalidExpressionType(
          'Lambda literal found, but non-function-type `${type.value}` expected.',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }
    final functionType = type.mapValue((t) => t as hir.FunctionCandyType);

    final parameters = <String, hir.CandyType>{};
    final declaredParameters = expression.valueParameters;
    final errors = <ReportedCompilerError>[];
    if (functionType.isSome) {
      final typeParameters = functionType.value.parameterTypes;
      if (typeParameters.length == 1 && declaredParameters.isEmpty) {
        parameters['it'] = typeParameters.single;
      } else if (declaredParameters.isNotEmpty) {
        if (declaredParameters.length != typeParameters.length) {
          return Error([
            CompilerError.invalidExpressionType(
              'Function with ${typeParameters.length} parameters expected, but ${declaredParameters.length} are declared.',
              location: ErrorLocation(
                resourceId,
                SourceSpan(
                  declaredParameters.first.span.start,
                  declaredParameters.last.span.end,
                ),
              ),
            ),
          ]);
        }

        for (final i in typeParameters.indices) {
          final typeParameter = typeParameters[i];
          final declaredParameter = declaredParameters[i];
          final name = declaredParameter.name.name;

          if (declaredParameter.type != null) {
            final hirType = astTypeToHirType(
              queryContext,
              Tuple2(moduleId, declaredParameter.type),
            );
            if (!isAssignableTo(queryContext, Tuple2(typeParameter, hirType))) {
              errors.add(CompilerError.invalidExpressionType(
                'Declared type `$hirType` is not assignable to expected type `${declaredParameter.type}`.',
                location:
                    ErrorLocation(resourceId, declaredParameter.type.span),
              ));
            }

            parameters[name] = hirType;
          } else {
            parameters[name] = typeParameter;
          }
        }
      } else {
        return Error([
          CompilerError.lambdaParametersMissing(
            "Lambda was inferred to have ${typeParameters.length} parameters, but those aren't declared.",
            location: ErrorLocation(resourceId, expression.span),
          ),
        ]);
      }
    } else {
      for (final parameter in declaredParameters) {
        var type = hir.CandyType.any;
        if (parameter.type == null) {
          errors.add(CompilerError.lambdaParameterTypeRequired(
            "Lambda parameter type can't be inferred.",
            location: ErrorLocation(resourceId, parameter.span),
          ));
        } else {
          type =
              astTypeToHirType(queryContext, Tuple2(moduleId, parameter.type));
        }
        parameters[parameter.name.name] = type;
      }
    }
    if (errors.isNotEmpty) return Error(errors);

    final identifiers = {
      ...parameters,
      if (functionType.valueOrNull?.receiverType != null)
        'this': functionType.value.receiverType,
    }.mapValues((k, v) => hir.Identifier.parameter(getId(expression), k, v));
    final lambdaContext =
        LambdaContext(this, getId(expression), None(), identifiers);

    final returnType = functionType.mapValue((t) => t.returnType);
    final astExpressions = expression.expressions;
    final hirExpressions =
        <Result<hir.Expression, List<ReportedCompilerError>>>[];
    for (var i = 0; i < astExpressions.length; i++) {
      final astExpression = astExpressions[i];

      if (i == astExpressions.length - 1) {
        var lowered = lambdaContext
            .innerExpressionContext(expressionType: Some(returnType.value))
            .lowerUnambiguous(astExpression);
        if (lowered is Ok && lowered.value.type != hir.CandyType.never) {
          lowered = Ok(
            hir.Expression.return_(getId(), getId(expression), lowered.value),
          );
        }
        hirExpressions.add(lowered);
        break;
      }

      final result = lambdaContext
          .innerExpressionContext(forwardsIdentifiers: true)
          .lowerUnambiguous(astExpression);
      hirExpressions.add(result);
    }

    final mergedExpressions = hirExpressions.merge();
    if (mergedExpressions is Error) return mergedExpressions;
    final expressions = mergedExpressions.value;

    var actualReturnType = returnType.valueOrNull;
    if (actualReturnType == null) {
      final visitor = ReturnExpressionVisitor();
      for (final expression in expressions) {
        expression.accept(visitor);
      }
      actualReturnType = hir.CandyType.union(visitor.returnTypes.toList());
    }

    return Ok([
      hir.Expression.literal(
        getId(expression),
        hir.Literal.lambda(
          // This only works because Dart maintains the insertion order of
          // maps.
          parameters.entries
              .map((e) => hir.LambdaLiteralParameter(e.key, e.value))
              .toList(),
          expressions,
          actualReturnType,
          functionType.valueOrNull?.receiverType,
        ),
      ),
    ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerIdentifier(
    ast.Identifier expression,
  ) {
    final name = expression.value.name;

    final identifiers = resolveIdentifier(name);
    if (identifiers.isEmpty) {
      throw CompilerError.undefinedIdentifier(
        "Couldn't resolve identifier `$name`.",
        location: ErrorLocation(resourceId, expression.value.span),
      );
    }

    return Ok([
      for (final identifier in identifiers)
        hir.Expression.identifier(getId(expression), identifier),
    ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerLoop(
    ast.LoopExpression loop,
  ) {
    final loopContext = LoopContext(this, getId(loop), None());
    final loweredBody = loop.body.expressions.map((expression) {
      return loopContext
          .innerExpressionContext(forwardsIdentifiers: true)
          .lowerUnambiguous(expression);
    }).merge();
    if (loweredBody is Error) return loweredBody;
    final body = loweredBody.value;

    var type = expressionType.valueOrNull;
    if (type == null) {
      final visitor = BreakExpressionVisitor();
      for (final expression in body) {
        expression.accept(visitor);
      }
      type = hir.CandyType.union(visitor.breakTypes.toList());
    }

    return Ok([hir.LoopExpression(getId(loop), body, type)]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerProperty(
    ast.PropertyDeclaration expression,
  ) {
    if (expression.accessors.isNotEmpty) {
      throw CompilerError.unsupportedFeature(
        'Accessors for local properties are not yet supported.',
        location: ErrorLocation(resourceId, expression.representativeSpan),
      );
    }

    final type = expression.type != null
        ? astTypeToHirType(queryContext, Tuple2(moduleId, expression.type))
        : null;

    final initializer = expression.initializer;
    if (initializer == null) {
      throw CompilerError.propertyInitializerMissing(
        'Local properties must have an initializer.',
        location: ErrorLocation(resourceId, expression.representativeSpan),
      );
    }

    return innerExpressionContext(expressionType: Option.of(type))
        .lowerUnambiguous(initializer)
        .mapValue((initializer) {
      final id = getId(expression);
      final name = expression.name.name;
      final actualType = type ?? initializer.type;

      addIdentifier(hir.LocalPropertyIdentifier(id, name, actualType));
      final result = hir.Expression.property(
        id,
        name,
        actualType,
        initializer,
        isMutable: expression.isMutable,
      );
      return [result];
    });
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerCall(
    ast.CallExpression expression,
  ) {
    final targetVariants = innerExpressionContext().lower(expression.target);
    if (targetVariants is Error) return targetVariants;

    final results = targetVariants.value.map((target) {
      if (target is hir.IdentifierExpression &&
          target.identifier is hir.PropertyIdentifier) {
        final identifier = target.identifier as hir.PropertyIdentifier;
        if (identifier.id.isFunction) {
          return lowerFunctionCall(expression, target);
        }
      }

      throw CompilerError.unsupportedFeature(
        'Callable expressions are not yet supported.',
        location: ErrorLocation(resourceId, expression.span),
      );
    });
    return results.merge();
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerFunctionCall(
    ast.CallExpression expression,
    hir.IdentifierExpression target,
  ) {
    assert(target.identifier is hir.PropertyIdentifier);
    final identifier = target.identifier as hir.PropertyIdentifier;

    final functionId = identifier.id;
    assert(functionId.isFunction);
    final functionHir = getFunctionDeclarationHir(queryContext, functionId);
    if (!isValidExpressionType(functionHir.returnType)) {
      return Error([
        CompilerError.invalidExpressionType(
          'Function has an invalid return type.',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }

    final errors = <ReportedCompilerError>[];

    // Attention: The map containing lowered arguments must retain their order
    // from the source code/AST. This currently works, because Dart's map
    // maintains insertion order.

    final parameters = functionHir.parameters;
    final parametersByName = parameters.associateBy((p) => p.name);
    final arguments = expression.arguments;
    final outOfOrderNamedArguments = <ast.Argument>[];
    final astArgumentMap = <String, ast.Argument>{};
    for (var i = 0; i < arguments.length; i++) {
      final argument = arguments[i];
      if (outOfOrderNamedArguments.isNotEmpty && argument.isPositional) {
        errors.add(CompilerError.unexpectedPositionalArgument(
          'At least one of the preceding arguments was named and not in the '
          'default order, hence positional arguments can no longer be used.',
          location: ErrorLocation(resourceId, argument.span),
          relatedInformation: [
            for (final arg in outOfOrderNamedArguments)
              ErrorRelatedInformation(
                location: ErrorLocation(resourceId, arg.span),
                message: "A named argument that's not in the default order.",
              ),
          ],
        ));
        continue;
      }

      if (i >= parameters.length) {
        errors.add(CompilerError.tooManyArguments(
          'Too many arguments.',
          location: ErrorLocation(resourceId, argument.span),
        ));
        continue;
      }

      final parameter = parameters[i];
      if (argument.isPositional) {
        astArgumentMap[parameter.name] = argument;
      } else {
        final parameterName = argument.name.name;
        assert(parameterName != null);
        if (astArgumentMap.containsKey(parameterName)) {
          errors.add(CompilerError.duplicateArgument(
            'Argument `$parameterName` was already given.',
            location: ErrorLocation(resourceId, argument.span),
          ));
          continue;
        }

        astArgumentMap[parameterName] = argument;
        if (parameter.name != parameterName) {
          outOfOrderNamedArguments.add(argument);
        }
      }
    }
    if (errors.isNotEmpty) return Error(errors);

    final hirArgumentMap = <String, hir.Expression>{};
    for (final entry in astArgumentMap.entries) {
      final name = entry.key;
      final value = entry.value.expression;

      final innerContext = innerExpressionContext(
        expressionType: Option.some(parametersByName[name].type),
      );
      final lowered = innerContext.lowerUnambiguous(value);
      if (lowered is Error) {
        errors.addAll(lowered.error);
        continue;
      }

      hirArgumentMap[name] = lowered.value;
    }
    if (errors.isNotEmpty) return Error(errors);

    return Ok([
      hir.Expression.functionCall(
        getId(expression),
        target,
        hirArgumentMap,
      ),
    ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerReturn(
    ast.ReturnExpression expression,
  ) {
    // The type of a `ReturnExpression` is `Never` and that is, by definition,
    // assignable to anything because it's a bottom type. So, we don't need to
    // check that.

    final resolvedScope = resolveReturn(None());
    if (resolvedScope is None) {
      return Error([
        CompilerError.invalidLabel(
          'Return label is invalid.',
          location: ErrorLocation(resourceId, expression.returnKeyword.span),
        ),
      ]);
    }

    if (expression.expression == null) {
      return Ok([
        hir.Expression.return_(getId(expression), resolvedScope.value.first),
      ]);
    }

    return innerExpressionContext(expressionType: resolvedScope.value.second)
        .lowerUnambiguous(expression.expression)
        .mapValue((hirExpression) => [
              hir.Expression.return_(
                getId(expression),
                resolvedScope.value.first,
                hirExpression,
              ),
            ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerBreak(
    ast.BreakExpression expression,
  ) {
    // The type of a `BreakExpression` is `Never` and that is, by definition,
    // assignable to anything because it's a bottom type. So, we don't need to
    // check that.

    final resolvedScope = resolveBreak(None());
    if (resolvedScope is None) {
      return Error([
        CompilerError.invalidLabel(
          'Break label is invalid.',
          location: ErrorLocation(resourceId, expression.breakKeyword.span),
        ),
      ]);
    }

    if (expression.expression == null) {
      return Ok([
        hir.Expression.break_(getId(expression), resolvedScope.value.first),
      ]);
    }

    return innerExpressionContext(expressionType: resolvedScope.value.second)
        .lowerUnambiguous(expression.expression)
        .mapValue((hirExpression) => [
              hir.Expression.break_(
                getId(expression),
                resolvedScope.value.first,
                hirExpression,
              ),
            ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerContinue(
    ast.ContinueExpression expression,
  ) {
    // The type of a `ContinueExpression` is `Never` and that is, by definition,
    // assignable to anything because it's a bottom type. So, we don't need to
    // check that.

    final resolvedScope = resolveContinue(None());
    if (resolvedScope is None) {
      return Error([
        CompilerError.invalidLabel(
          'Continue label is invalid.',
          location: ErrorLocation(resourceId, expression.continueKeyword.span),
        ),
      ]);
    }

    return Ok([
      hir.Expression.continue_(getId(expression), resolvedScope.value),
    ]);
  }
}
