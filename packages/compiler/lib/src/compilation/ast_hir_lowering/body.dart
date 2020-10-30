import 'dart:io';

import 'package:dartx/dartx.dart';
import 'package:meta/meta.dart';
import 'package:parser/parser.dart' as ast;
import 'package:parser/parser.dart' show SourceSpan;

import '../../constants.dart';
import '../../errors.dart';
import '../../query.dart';
import '../../utils.dart';
import '../ast.dart';
import '../hir.dart' as hir;
import '../hir.dart';
import '../hir/ids.dart';
import '../ids.dart';
import 'declarations/class.dart';
import 'declarations/constructor.dart';
import 'declarations/declarations.dart';
import 'declarations/function.dart';
import 'declarations/impl.dart';
import 'declarations/module.dart';
import 'declarations/property.dart';
import 'declarations/trait.dart';
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
    if (node.identifier is hir.ReflectionIdentifier) {
      final base = (node.identifier as hir.ReflectionIdentifier).base;
      if (base != null) return base.accept(this);
    } else if (node.identifier is hir.PropertyIdentifier) {
      final base = (node.identifier as hir.PropertyIdentifier).base;
      if (base != null) return base.accept(this);
      final receiver = (node.identifier as hir.PropertyIdentifier).receiver;
      if (receiver != null) return receiver.accept(this);
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
  Option<hir.Expression> visitConstructorCallExpression(
    ConstructorCallExpression node,
  ) {
    if (node.id == id) return Some(node);
    for (final argument in node.valueArguments.values) {
      final result = argument.accept(this);
      if (result is Some) return result;
    }
    return None();
  }

  @override
  Option<hir.Expression> visitReturnExpression(ReturnExpression node) {
    if (node.id == id) return Some(node);
    if (node.expression != null) return node.expression.accept(this);
    return None();
  }

  @override
  Option<hir.Expression> visitIfExpression(IfExpression node) {
    if (node.id == id) return Some(node);
    final result = node.condition.accept(this);
    if (result is Some) return result;
    for (final expression in node.thenBody) {
      final result = expression.accept(this);
      if (result is Some) return result;
    }
    for (final expression in node.elseBody) {
      final result = expression.accept(this);
      if (result is Some) return result;
    }
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
  Option<hir.Expression> visitWhileExpression(WhileExpression node) {
    if (node.id == id) return Some(node);
    final result = node.condition.accept(this);
    if (result is Some) return result;
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

  @override
  Option<hir.Expression> visitAssignmentExpression(AssignmentExpression node) {
    if (node.id == id) return Some(node);
    final result = node.left.accept(this);
    if (result is Some) return result;
    return node.right.accept(this);
  }

  @override
  Option<hir.Expression> visitIsExpression(IsExpression node) {
    if (node.id == id) return Some(node);
    return node.instance.accept(this);
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
  Option<hir.CandyType> get thisType;

  Option<Context> get parent;

  Option<hir.CandyType> get expressionType;
  bool isValidExpressionType(hir.CandyType type) {
    return expressionType.when(
      some: (expressionType) => isAssignableTo(
        queryContext,
        Tuple2(
          type.bakeThisType(thisType.valueOrNull),
          expressionType.bakeThisType(thisType.valueOrNull),
        ),
      ),
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
    } else if (expression is ast.GroupExpression) {
      result = lower(expression.expression);
    } else if (expression is ast.PropertyDeclaration) {
      result = lowerProperty(expression);
    } else if (expression is ast.NavigationExpression) {
      result = lowerNavigation(expression);
    } else if (expression is ast.CallExpression) {
      result = lowerCall(expression);
    } else if (expression is ast.ReturnExpression) {
      result = lowerReturn(expression);
    } else if (expression is ast.IfExpression) {
      result = lowerIf(expression);
    } else if (expression is ast.LoopExpression) {
      result = lowerLoop(expression);
    } else if (expression is ast.WhileExpression) {
      result = lowerWhile(expression);
    } else if (expression is ast.BreakExpression) {
      result = lowerBreak(expression);
    } else if (expression is ast.ContinueExpression) {
      result = lowerContinue(expression);
    } else if (expression is ast.PrefixExpression) {
      result = lowerPrefixExpression(expression);
    } else if (expression is ast.BinaryExpression) {
      result = lowerBinaryExpression(expression);
    } else if (expression is ast.IsExpression) {
      result = lowerIsExpression(expression);
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported expression: $expression (`${expression.runtimeType}`).',
        location: ErrorLocation(resourceId, expression.span),
      );
    }

    assert(result != null);
    if (result is Error) {
      assert(result.error.isNotEmpty);
      return result;
    }

    assert(result.value.isNotEmpty);
    final actualResults =
        result.value.where((r) => isValidExpressionType(r.type));
    if (actualResults.isEmpty) {
      final possibleTypes = {
        for (final variant in result.value) variant.type,
      }.map((t) => '`$t`').toList().join(' or ');
      return Error([
        CompilerError.invalidExpressionType(
          'Expected type `${expressionType.value}`, got $possibleTypes.',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }
    return Ok(actualResults.toList());
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
  Option<hir.CandyType> get thisType => parent.value.thisType;

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
  ContextContext(this.queryContext, this.declarationId, this.thisType)
      : assert(queryContext != null),
        assert(declarationId != null),
        assert(thisType != null);

  @override
  final QueryContext queryContext;
  @override
  final DeclarationId declarationId;
  @override
  final Option<hir.CandyType> thisType;

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
    hir.CandyType thisTypeOrResolved() {
      final parent = declarationId.parent;
      if (parent.isClass) return thisType.value;
      if (parent.isImpl) {
        final typeModuleId =
            getImplDeclarationHir(queryContext, parent).type.virtualModuleId;
        if (moduleIdToDeclarationId(
          queryContext,
          typeModuleId,
        ).isClass) return thisType.value;
      } else if (parent.isTrait || parent.isImpl) {
        return hir.CandyType.this_();
      }
    }

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
        return [hir.Identifier.this_(thisTypeOrResolved())];
      }
      return [];
    }

    // resolve `field` in a getter/setter
    // TODO(JonasWanke): resolve `field` in property accessors

    hir.Identifier convertDeclarationId(
      DeclarationId id, [
      hir.Expression receiver,
    ]) {
      if (id.isModule || id.isTrait || id.isClass) {
        return hir.Identifier.reflection(id);
      } else if (id.isFunction) {
        final functionHir = getFunctionDeclarationHir(queryContext, id);
        return hir.Identifier.property(
          id,
          functionHir.functionType,
          isMutable: false,
          receiver: receiver,
        );
      } else if (id.isProperty) {
        final propertyHir = getPropertyDeclarationHir(queryContext, id);
        return hir.Identifier.property(
          id,
          propertyHir.type,
          isMutable: propertyHir.isMutable,
          receiver: receiver,
        );
      } else {
        throw CompilerError.unsupportedFeature(
          "Matched identifier `$name`, but it's not a module, trait, class, function, or property.",
        );
      }
    }

    // search properties/functions available on `this`
    // TODO(JonasWanke): cleaner implementation, like `query fun getAllInstanceIdentifiersForType(type: hir.CandyType)`
    Iterable<DeclarationId> getInstanceDeclarations() sync* {
      if (declarationId.isFunction) {
        final functionHir =
            getFunctionDeclarationHir(queryContext, declarationId);
        if (functionHir.isStatic) return;
      } else if (declarationId.isProperty) {
        final propertyHir =
            getPropertyDeclarationHir(queryContext, declarationId);
        if (propertyHir.isStatic) return;
      } else {
        throw CompilerError.unsupportedFeature(
          "Tried lowering an identifier in a body that's neither in a function nor in a property.",
          location: ErrorLocation(resourceId),
        );
      }

      assert(declarationId.hasParent);
      final parentId = declarationId.parent;
      assert(parentId.isTrait || parentId.isImpl || parentId.isClass);
      yield parentId;

      Iterable<DeclarationId> walkHierarchy(DeclarationId typeId) sync* {
        assert(typeId.isTrait || typeId.isClass);
        yield typeId;

        final impls = getAllImplsForTraitOrClass(queryContext, typeId);
        yield* impls;

        yield* impls
            .map((impl) => getImplDeclarationHir(queryContext, impl))
            .expand((impl) => impl.traits)
            .map((trait) =>
                moduleIdToDeclarationId(queryContext, trait.virtualModuleId))
            .expand(walkHierarchy);

        if (typeId.isTrait) {
          final traitHir = getTraitDeclarationHir(queryContext, typeId);
          yield* traitHir.upperBounds
              .map((b) =>
                  moduleIdToDeclarationId(queryContext, b.virtualModuleId))
              .expand(walkHierarchy);
        }
      }

      if (parentId.isTrait || parentId.isClass) {
        yield* walkHierarchy(parentId);
      } else {
        assert(parentId.isImpl);
        final implHir = getImplDeclarationHir(queryContext, parentId);
        yield* walkHierarchy(
          moduleIdToDeclarationId(queryContext, implHir.type.virtualModuleId),
        );
      }
    }

    final matches = getInstanceDeclarations()
        .toSet()
        .expand((id) => getInnerDeclarationIds(queryContext, id))
        .where((id) {
      if (id.isProperty) {
        final propertyHir = getPropertyDeclarationHir(queryContext, id);
        return propertyHir.name == name && !propertyHir.isStatic;
      } else if (id.isFunction) {
        final functionHir = getFunctionDeclarationHir(queryContext, id);
        return functionHir.name == name && !functionHir.isStatic;
      } else {
        return false;
      }
    }).map((id) => convertDeclarationId(
              id,
              hir.Expression.identifier(
                getId(),
                hir.Identifier.this_(thisTypeOrResolved()),
              ),
            ));
    // TODO(marcelgarus): Maybe be more careful when choosing a match.
    if (matches.isNotEmpty) return [matches.first];

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
    final functionHir = getFunctionDeclarationHir(queryContext, id);
    final parent = ContextContext(
      queryContext,
      id,
      functionHir.isStatic ? None() : getThisType(queryContext, id.parent),
    );
    final functionAst = getFunctionDeclarationAst(queryContext, id);
    final identifiers = {
      for (final parameter in functionAst.valueParameters)
        parameter.name.name: hir.Identifier.parameter(
          parent.getId(parameter),
          parameter.name.name,
          astTypeToHirType(
            parent.queryContext,
            Tuple2(parent.declarationId, parameter.type),
          ),
        ),
    };

    return FunctionContext._(
      parent,
      identifiers,
      functionHir.returnType,
      functionAst.body,
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

  static Option<hir.CandyType> getThisType(
    QueryContext queryContext,
    DeclarationId id,
  ) {
    if (id.isTrait) {
      return Some(getTraitDeclarationHir(queryContext, id).thisType);
    } else if (id.isImpl) {
      return Some(getImplDeclarationHir(queryContext, id).type);
    } else if (id.isClass) {
      return Some(getClassDeclarationHir(queryContext, id).thisType);
    } else {
      return None();
    }
  }

  @override
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
      // TODO(marcelgarus): Bake the return type.
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
    final parent = ContextContext(
      queryContext,
      id,
      FunctionContext.getThisType(queryContext, id.parent),
    );
    final ast = getPropertyDeclarationAst(queryContext, id);

    final type = Option.of(ast.type).mapValue(
        (t) => astTypeToHirType(queryContext, Tuple2(parent.declarationId, t)));

    return PropertyContext._(
      parent,
      type,
      ast.initializer,
    );
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

class IfContext extends InnerContext {
  IfContext(
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
  Option<Tuple2<DeclarationLocalId, Option<hir.CandyType>>> resolveReturn(
    Option<String> label,
  ) {
    if (label is None ||
        label == this.label ||
        this.label is None && label == Some('if')) {
      return Some(Tuple2(id, parent.value.expressionType));
    }
    return parent.flatMapValue((context) => context.resolveBreak(label));
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
      literal = hir.Literal.boolean(token.value);
    } else if (token is ast.IntLiteralToken) {
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
              Tuple2(declarationId, declaredParameter.type),
            );

            // TODO(JonasWanke): resolve correct `This`-type
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
          type = astTypeToHirType(
            queryContext,
            Tuple2(declarationId, parameter.type),
          );
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
      return Error([
        CompilerError.undefinedIdentifier(
          "Couldn't resolve identifier `$name`.",
          location: ErrorLocation(resourceId, expression.value.span),
        ),
      ]);
    }

    return Ok([
      for (final identifier in identifiers)
        hir.Expression.identifier(getId(expression), identifier),
    ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerIf(
    ast.IfExpression theIf,
  ) {
    final loweredCondition =
        innerExpressionContext(expressionType: Some(CandyType.bool))
            .lowerUnambiguous(theIf.condition);
    if (loweredCondition is Error) return Error(loweredCondition.error);
    final condition = loweredCondition.value;

    final thenContext = IfContext(this, getId(theIf), None());
    final loweredThenBody = theIf.thenBody.expressions.map((expression) {
      return thenContext
          .innerExpressionContext(forwardsIdentifiers: true)
          .lowerUnambiguous(expression);
    }).merge();
    if (loweredThenBody is Error) return loweredThenBody;
    final thenBody = loweredThenBody.value;

    final elseBody = <Expression>[];
    if (theIf.elseKeyword != null) {
      assert(theIf.elseBody != null);
      final elseContext = IfContext(this, getId(theIf), None());
      final loweredElseBody = theIf.elseBody.expressions.map((expression) {
        return elseContext
            .innerExpressionContext(forwardsIdentifiers: true)
            .lowerUnambiguous(expression);
      }).merge();
      if (loweredElseBody is Error) return loweredElseBody;
      elseBody.addAll(loweredElseBody.value);
    }

    final type = expressionType.valueOrNull ??
        hir.CandyType.union({
          if (thenBody.isNotEmpty) thenBody.last.type,
          if (elseBody.isNotEmpty) elseBody.last.type,
        }.toList());

    return Ok([
      hir.IfExpression(getId(theIf), condition, thenBody, elseBody, type),
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

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerWhile(
    ast.WhileExpression whileLoop,
  ) {
    final loopContext = LoopContext(this, getId(whileLoop), None());

    final loweredCondition = loopContext
        .innerExpressionContext(expressionType: Some(CandyType.bool))
        .lowerUnambiguous(whileLoop.condition);
    if (loweredCondition is Error) return loweredCondition.mapValue((e) => [e]);
    final condition = loweredCondition.value;

    final loweredBody = whileLoop.body.expressions.map((expression) {
      return loopContext
          .innerExpressionContext(forwardsIdentifiers: true)
          .lowerUnambiguous(expression);
    }).merge();
    if (loweredBody is Error) return loweredBody;
    final body = loweredBody.value;

    // TODO(marcelgarus): Implement while-else constructs that can also evaluate to something other than unit.
    return Ok([
      hir.WhileExpression(getId(whileLoop), condition, body, CandyType.unit),
    ]);
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
        ? astTypeToHirType(
            queryContext,
            Tuple2(declarationId, expression.type),
          )
        : null;

    final initializer = expression.initializer;
    if (initializer == null) {
      throw CompilerError.propertyInitializerMissing(
        'Local properties must have an initializer.',
        location: ErrorLocation(resourceId, expression.representativeSpan),
      );
    }

    final id = getId(expression);
    final name = expression.name.name;
    return innerExpressionContext(expressionType: Option.of(type))
        .lowerUnambiguous(initializer)
        .mapValue((initializer) {
      final actualType = type ?? initializer.type;

      addIdentifier(hir.LocalPropertyIdentifier(
        id,
        name,
        actualType,
        expression.isMutable,
      ));
      final result = hir.Expression.property(
        id,
        name,
        actualType,
        initializer,
        isMutable: expression.isMutable,
      );
      return [result];
    }).mapError((error) {
      addIdentifier(hir.LocalPropertyIdentifier(
        id,
        name,
        type ?? hir.CandyType.any,
        expression.isMutable,
      ));
      return error;
    });
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerNavigation(
    ast.NavigationExpression expression,
  ) {
    final loweredTarget =
        innerExpressionContext().lowerUnambiguous(expression.target);
    if (loweredTarget is Error) return Error(loweredTarget.error);
    final target = loweredTarget.value;
    final name = expression.name.name;

    List<hir.PropertyIdentifier> getMatchesForType(hir.UserCandyType type) {
      final receiverId =
          moduleIdToDeclarationId(queryContext, type.virtualModuleId);
      return getInnerDeclarationIds(queryContext, receiverId)
          .where((id) => id.simplePath.last.nameOrNull == name)
          .mapNotNull((id) {
        if (id.isModule || id.isTrait || id.isClass) return null;
        if (id.isProperty) {
          final propertyHir = getPropertyDeclarationHir(queryContext, id);
          if (propertyHir.isStatic) return null;
          return hir.PropertyIdentifier(
            id,
            propertyHir.type.bakeThisType(type),
            base: target,
            receiver: target,
          );
        } else if (id.isFunction) {
          final functionHir = getFunctionDeclarationHir(queryContext, id);
          if (functionHir.isStatic) return null;
          return hir.PropertyIdentifier(
            id,
            functionHir.functionType.bakeThisType(type),
            base: target,
            receiver: target,
          );
        } else {
          throw CompilerError.internalError(
            'Identifier resolved to an invalid declaration type: `$id`.',
            location: ErrorLocation(resourceId, expression.name.span),
          );
        }
      }).toList();
    }

    Result<List<hir.Expression>, List<ReportedCompilerError>> lower(
        hir.CandyType type) {
      return type.map(
        user: (type) {
          final matches = getMatchesForType(type)
              .map((m) => hir.IdentifierExpression(getId(expression), m));
          if (matches.isEmpty) {
            final receiverId =
                moduleIdToDeclarationId(queryContext, type.virtualModuleId);
            return Error([
              CompilerError.unknownInnerDeclaration(
                // TODO(JonasWanke): better error description
                "Declaration `$receiverId` doesn't contain an instance declaration called '$name'.",
                location: ErrorLocation(resourceId, expression.name.span),
              ),
            ]);
          }
          return Ok(matches.toList());
        },
        this_: (_) {
          final type =
              getPropertyDeclarationParentAsType(queryContext, declarationId)
                  .value;
          final matches = getMatchesForType(type)
              .map((m) => hir.IdentifierExpression(getId(expression), m));
          if (matches.isEmpty) {
            return Error([
              CompilerError.unknownInnerDeclaration(
                // TODO(JonasWanke): better error description
                "`This` doesn't contain an instance declaration called '$name'.",
                location: ErrorLocation(resourceId, expression.name.span),
              ),
            ]);
          }
          return Ok(matches.toList());
        },
        tuple: (type) {
          const fieldNames = [
            'first',
            'second',
            'third',
            'fourth',
            'fifth',
            'sixth',
            'seventh',
            'eight',
            'nineth',
            'tenth',
          ];
          final fieldIndex = fieldNames.indexOf(name);
          final tupleSize = type.items.length;
          if (fieldIndex >= 0 && fieldIndex < tupleSize) {
            return Ok([
              hir.Expression.identifier(
                getId(expression),
                hir.Identifier.property(
                  DeclarationId(ResourceId(
                    PackageId.core,
                    '$srcDirectoryName/primitives$candyFileExtension',
                  ))
                      .inner(DeclarationPathData.class_('Tuple$tupleSize'))
                      .inner(DeclarationPathData.property(name)),
                  type.items[fieldIndex],
                  base: target,
                  receiver: target,
                ),
              ),
            ]);
          }

          return Error([
            CompilerError.unsupportedFeature(
              "Tuple type `$type` doesn't contain a property called '$name'.",
              location: ErrorLocation(resourceId, expression.name.span),
            ),
          ]);
        },
        function: (type) {
          return Error([
            CompilerError.unsupportedFeature(
              "Function type `$type` doesn't contain a property called '$name'.",
              location: ErrorLocation(resourceId, expression.name.span),
            ),
          ]);
        },
        union: (type) {
          return Error([
            CompilerError.unsupportedFeature(
              "Union type `$type` doesn't contain a property called '$name'.",
              location: ErrorLocation(resourceId, expression.name.span),
            ),
          ]);
        },
        intersection: (type) {
          if (type.types.any((t) => t is! hir.UserCandyType)) {
            return Error([
              CompilerError.unsupportedFeature(
                'Property access on expressions whose type is a non-simple intersection type is not yet supported.',
                location: ErrorLocation(resourceId, expression.name.span),
              ),
            ]);
          }

          final matches = type.types
              .map((t) => getMatchesForType(type as hir.UserCandyType));
          final nonEmptyMatches = matches.where((m) => m.isNotEmpty);
          if (nonEmptyMatches.isEmpty) {
            return Error([
              CompilerError.unsupportedFeature(
                'No variants of the intersection type define a property or function with that name, which is not supported yet.',
                location: ErrorLocation(resourceId, expression.name.span),
              ),
            ]);
          } else if (nonEmptyMatches.length > 1) {
            return Error([
              CompilerError.unsupportedFeature(
                'Multiple variants of the intersection type define properties/functions with this name, which is not supported yet.',
                location: ErrorLocation(resourceId, expression.name.span),
              ),
            ]);
          }

          final finalMatches = nonEmptyMatches.single
              .map((i) => hir.Expression.identifier(getId(expression), i))
              .toList();
          return Ok(finalMatches);
        },
        parameter: (type) => lower(getTypeParameterBound(queryContext, type)),
        reflection: (targetType) {
          final targetId = targetType.declarationId;
          // Only `IdentifierExpression`s containing a `ReflectionIdentifier` can
          // lead to a reflection type.
          final reflectionTarget = target as hir.IdentifierExpression;

          final innerIds = getInnerDeclarationIds(queryContext, targetId);
          final matches = innerIds
              .where((id) => id.simplePath.last.nameOrNull == name)
              .map((id) {
            hir.Identifier identifier;
            if (id.isModule || id.isTrait || id.isClass) {
              identifier = hir.Identifier.reflection(id, reflectionTarget);
            } else if (id.isProperty) {
              final propertyHir = getPropertyDeclarationHir(queryContext, id);
              identifier = propertyHir.isStatic
                  ? hir.Identifier.property(
                      id,
                      propertyHir.type,
                      base: reflectionTarget,
                    )
                  : hir.Identifier.reflection(id, reflectionTarget);
            } else if (id.isFunction) {
              final functionHir = getFunctionDeclarationHir(queryContext, id);
              identifier = functionHir.isStatic
                  ? hir.Identifier.property(
                      id,
                      functionHir.functionType,
                      base: reflectionTarget,
                    )
                  : hir.Identifier.reflection(id, reflectionTarget);
            } else {
              throw CompilerError.internalError(
                'Identifier resolved to an invalid declaration type: `$id`.',
                location: ErrorLocation(resourceId, expression.name.span),
              );
            }
            return hir.Expression.identifier(getId(expression), identifier);
          });
          if (matches.isEmpty) {
            return Error([
              CompilerError.unknownInnerDeclaration(
                // TODO(JonasWanke): better error description
                "Declaration `$targetId` doesn't contain an inner declaration called '$name'.",
                location: ErrorLocation(resourceId, expression.name.span),
              ),
            ]);
          }
          return Ok(matches.toList());
        },
      );
    }

    return lower(target.type);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerCall(
    ast.CallExpression expression,
  ) {
    final targetVariants = innerExpressionContext().lower(expression.target);
    if (targetVariants is Error) return targetVariants;

    final results = targetVariants.value.map((target) {
      // Function call.
      if (target is hir.IdentifierExpression &&
          target.identifier is hir.PropertyIdentifier) {
        final identifier = target.identifier as hir.PropertyIdentifier;
        if (identifier.id.isFunction) {
          return lowerFunctionCall(expression, target);
        }
      }

      // Constructor call.
      if (target is hir.IdentifierExpression &&
          target.identifier is hir.ReflectionIdentifier) {
        // TODO(marcelgarus): Ensure this is a constructor call.
        return lowerConstructorCall(expression, target);
      }

      throw CompilerError.unsupportedFeature(
        'Callable expressions are not yet supported (target $target).',
        location: ErrorLocation(resourceId, expression.span),
      );
    });
    return results.merge();
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerFunctionCall(
    ast.CallExpression expression,
    hir.IdentifierExpression target,
  ) {
    assert(target != null);
    assert(target.identifier is hir.PropertyIdentifier);
    final identifier = target.identifier as hir.PropertyIdentifier;

    final functionId = identifier.id;
    assert(functionId.isFunction);

    var functionHir = getFunctionDeclarationHir(queryContext, functionId);
    if (functionHir.typeParameters.length !=
        (expression.typeArguments?.arguments?.length ?? 0)) {
      return Error([
        CompilerError.wrongNumberOfTypeArguments(
          'Function expected ${functionHir.typeParameters.length} parameters, '
          'but you provided ${expression.typeArguments?.arguments?.length ?? 0}.',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }

    final typeParameters = functionHir.typeParameters
        .map((p) => CandyType.parameter(p.name, functionId))
        .toList();
    final typeArguments = expression.typeArguments?.arguments
            ?.map((a) =>
                astTypeToHirType(queryContext, Tuple2(declarationId, a.type)))
            ?.toList() ??
        [];
    final genericsMap = Map.fromEntries(
        typeParameters.zip<CandyType, MapEntry<CandyType, CandyType>>(
            typeArguments, (a, b) => MapEntry(a, b)));
    functionHir = functionHir.copyWith(
      valueParameters: functionHir.valueParameters
          .map((parameter) => parameter.copyWith(
              type: parameter.type.bakeGenerics(genericsMap)))
          .toList(),
      returnType: functionHir.returnType.bakeGenerics(genericsMap),
    );

    if (!isValidExpressionType(functionHir.returnType)) {
      return Error([
        CompilerError.internalError(
          'Function ${functionHir.name} has an invalid return type: ${functionHir.returnType}. '
          'Expected: $expressionType',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }

    final errors = <ReportedCompilerError>[];

    // Attention: The map containing lowered arguments must retain their order
    // from the source code/AST. This currently works, because Dart's map
    // maintains insertion order.

    final parameters = functionHir.valueParameters;
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

    final missingArguments =
        parametersByName.keys.where((p) => !astArgumentMap.containsKey(p));
    if (missingArguments.isNotEmpty) {
      final argsMessage = missingArguments.map((a) => '`$a`').join(', ');
      return Error([
        CompilerError.missingArguments(
          'The following arguments were not supplied: $argsMessage.',
          location: ErrorLocation(resourceId, expression.leftParenthesis.span),
        ),
      ]);
    }

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
        expression.typeArguments?.arguments
                ?.map((argument) => astTypeToHirType(
                    queryContext, Tuple2(declarationId, argument.type)))
                ?.toList() ??
            [],
        hirArgumentMap,
        functionHir.returnType,
      ),
    ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>>
      lowerConstructorCall(
    ast.CallExpression expression,
    hir.IdentifierExpression target,
  ) {
    assert(target != null);
    assert(target.identifier is hir.ReflectionIdentifier,
        'target.identifier is not a ReflectionIdentifier: ${target.identifier}');

    final classId = (target.identifier as hir.ReflectionIdentifier).id;
    // ignore: non_constant_identifier_names
    final class_ = getClassDeclarationHir(queryContext, classId);
    final constructorId =
        class_.innerDeclarationIds.singleWhere((id) => id.isConstructor);

    final typeParameters = class_.typeParameters
        .map((p) => CandyType.parameter(p.name, classId))
        .toList();
    final typeArguments = expression.typeArguments?.arguments
            ?.map((a) =>
                astTypeToHirType(queryContext, Tuple2(declarationId, a.type)))
            ?.toList() ??
        [];
    final genericsMap = Map.fromEntries(
        typeParameters.zip<CandyType, MapEntry<CandyType, CandyType>>(
            typeArguments, (a, b) => MapEntry(a, b)));

    final fields = class_.innerDeclarationIds
        .where((id) => id.isProperty)
        .map((id) => getPropertyDeclarationHir(queryContext, id))
        .where((field) => !field.isStatic)
        .toList();
    final valueParameterTypes =
        fields.map((field) => field.type.bakeGenerics(genericsMap)).toList();
    final valueArguments = expression.arguments;

    final returnType = class_.thisType.bakeGenerics(genericsMap);

    if (typeParameters.length != (typeArguments?.length ?? 0)) {
      return Error([
        CompilerError.wrongNumberOfTypeArguments(
          'Constructor expected ${typeParameters.length} type parameters, '
          'but you provided ${typeArguments?.length ?? 0}.',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }

    if (!isValidExpressionType(returnType)) {
      return Error([
        CompilerError.invalidExpressionType(
          'Constructor has an invalid return type: $returnType. Expected: $expressionType',
          location: ErrorLocation(resourceId, expression.span),
        ),
      ]);
    }

    if (valueParameterTypes.length < valueArguments.length) {
      return Error([
        CompilerError.tooManyArguments(
          'Too many constructor arguments.',
          location: ErrorLocation(resourceId, expression.span),
        )
      ]);
    }

    if (valueParameterTypes.length > valueArguments.length) {
      return Error([
        CompilerError.missingArguments(
          'Too few constructor arguments.',
          location: ErrorLocation(resourceId, expression.span),
        )
      ]);
    }

    final loweredArguments = [
      for (var i = 0; i < valueArguments.length; i++)
        innerExpressionContext(
          expressionType: Option.some(valueParameterTypes[i]),
        ).lowerUnambiguous(valueArguments[i].expression),
    ].merge();
    if (loweredArguments is Error) return loweredArguments;
    final arguments = loweredArguments.value;

    return Ok([
      hir.Expression.constructorCall(
        getId(expression),
        class_,
        typeArguments,
        {
          for (var i = 0; i < arguments.length; i++)
            fields[i].name: arguments[i],
        },
        returnType,
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

  Result<List<hir.Expression>, List<ReportedCompilerError>>
      lowerPrefixExpression(
    ast.PrefixExpression expression,
  ) {
    final operatorType = expression.operatorToken.type;

    Result<List<hir.Expression>, List<ReportedCompilerError>> handle(
      hir.CandyType type,
      String functionName,
    ) {
      // TODO(JonasWanke): find a supertype that satisfies this trait
      final operandResult = innerExpressionContext(expressionType: Some(type))
          .lowerUnambiguous(expression.operand);
      if (operandResult is Error) return Error(operandResult.error);
      final operand = operandResult.value;

      return Ok([
        hir.Expression.functionCall(
          getId(expression),
          hir.Expression.identifier(
            getId(),
            hir.Identifier.property(
              moduleIdToDeclarationId(
                queryContext,
                type.virtualModuleId,
              ).inner(DeclarationPathData.function(functionName)),
              hir.CandyType.function(
                returnType: operand.type,
              ),
              isMutable: false,
              base: operand,
              receiver: operand,
            ),
          ),
          [],
          {},
          operand.type,
        ),
      ]);
    }

    if (operatorType == ast.OperatorTokenType.minus) {
      return handle(hir.CandyType.negate, 'negate');
    } else if (operatorType == ast.OperatorTokenType.exclamation) {
      return handle(hir.CandyType.opposite, 'opposite');
    } else {
      return Error([
        CompilerError.unsupportedFeature(
          'Unsupported prefix operator: ${expression.operatorToken.type}',
          location: ErrorLocation(resourceId, expression.operatorToken.span),
        ),
      ]);
    }
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>>
      lowerBinaryExpression(
    ast.BinaryExpression expression,
  ) {
    final operatorType = expression.operatorToken.type;

    Result<List<hir.Expression>, List<ReportedCompilerError>> handle(
      hir.CandyType type,
      String functionName, {
      @required hir.CandyType returnType,
    }) {
      final leftResult = innerExpressionContext(expressionType: Some(type))
          .lowerUnambiguous(expression.leftOperand);
      if (leftResult is Error) return Error(leftResult.error);
      final left = leftResult.value;

      // TODO(JonasWanke): find a supertype that satisfies this trait
      final right = innerExpressionContext(expressionType: Some(left.type))
          .lowerUnambiguous(expression.rightOperand);
      if (right is Error) return Error(right.error);

      return Ok([
        hir.Expression.functionCall(
          getId(expression),
          hir.Expression.identifier(
            getId(),
            hir.Identifier.property(
              moduleIdToDeclarationId(
                queryContext,
                type.virtualModuleId,
              ).inner(DeclarationPathData.function(functionName)),
              hir.CandyType.function(
                receiverType: left.type,
                parameterTypes: [left.type],
                returnType: returnType.bakeThisType(left.type),
              ),
              isMutable: false,
              base: left,
              receiver: left,
            ),
          ),
          [],
          {'other': right.value},
          returnType.bakeThisType(left.type),
        ),
      ]);
    }

    const comparisonOperators = {
      ast.OperatorTokenType.less: 'lessThan',
      ast.OperatorTokenType.lessEquals: 'lessThanOrEqual',
      ast.OperatorTokenType.greater: 'greaterThan',
      ast.OperatorTokenType.greaterEquals: 'greaterThanOrEqual',
    };

    if (operatorType == ast.OperatorTokenType.equals) {
      return lowerAssignment(expression);
    } else if (operatorType == ast.OperatorTokenType.plus) {
      return handle(
        hir.CandyType.add,
        'add',
        returnType: hir.CandyType.this_(),
      );
    } else if (operatorType == ast.OperatorTokenType.minus) {
      return handle(
        hir.CandyType.subtract,
        'subtract',
        returnType: hir.CandyType.this_(),
      );
    } else if (operatorType == ast.OperatorTokenType.asterisk) {
      return handle(
        hir.CandyType.multiply,
        'multiply',
        returnType: hir.CandyType.this_(),
      );
    } else if (operatorType == ast.OperatorTokenType.slash) {
      return handle(
        hir.CandyType.divide,
        'divide',
        returnType: hir.CandyType.float,
      );
    } else if (operatorType == ast.OperatorTokenType.tildeSlash) {
      return handle(
        hir.CandyType.divideTruncating,
        'divideTruncating',
        returnType: hir.CandyType.int,
      );
    } else if (operatorType == ast.OperatorTokenType.percent) {
      return handle(
        hir.CandyType.modulo,
        'modulo',
        returnType: hir.CandyType.this_(),
      );
    } else if (comparisonOperators.keys.contains(operatorType)) {
      final methodName = comparisonOperators[operatorType];
      return handle(
        hir.CandyType.comparable,
        methodName,
        returnType: hir.CandyType.bool,
      );
    } else if (operatorType == ast.OperatorTokenType.equalsEquals) {
      return handle(
        hir.CandyType.equals,
        'equals',
        returnType: hir.CandyType.bool,
      );
    } else if (operatorType == ast.OperatorTokenType.exclamationEquals) {
      return handle(
        hir.CandyType.equals,
        'notEquals',
        returnType: hir.CandyType.bool,
      );
    } else if (operatorType == ast.OperatorTokenType.ampersandAmpersand) {
      return handle(
        hir.CandyType.and,
        'and',
        returnType: hir.CandyType.bool,
      );
    } else if (operatorType == ast.OperatorTokenType.barBar) {
      return handle(
        hir.CandyType.or,
        'or',
        returnType: hir.CandyType.bool,
      );
    } else if (operatorType == ast.OperatorTokenType.dashGreater) {
      return handle(
        hir.CandyType.implies,
        'implies',
        returnType: hir.CandyType.bool,
      );
    } else {
      return Error([
        CompilerError.unsupportedFeature(
          'Unsupported binary operator: ${expression.operatorToken.type}',
          location: ErrorLocation(resourceId, expression.operatorToken.span),
        ),
      ]);
    }
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerIsExpression(
    ast.IsExpression expression,
  ) {
    final instanceResult =
        innerExpressionContext().lowerUnambiguous(expression.instance);
    if (instanceResult is Error) return Error(instanceResult.error);
    final instance = instanceResult.value;

    final type =
        astTypeToHirType(queryContext, Tuple2(declarationId, expression.type))
            .bakeThisType(thisType.valueOrNull);

    return Ok([
      hir.Expression.is_(
        getId(expression),
        instance,
        type,
        isNegated: expression.isNegated,
      ),
    ]);
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lowerAssignment(
    ast.BinaryExpression expression,
  ) {
    final leftExpression = lowerUnambiguous(expression.leftOperand);
    if (leftExpression is Error) return Error(leftExpression.error);
    final leftSome = leftExpression.value;
    if (leftSome is! IdentifierExpression) {
      return Error([
        CompilerError.invalidExpressionType("Can't assign to this expression: "
            '${leftSome.runtimeType} ($leftSome)'),
      ]);
    }
    final left = leftSome as IdentifierExpression;
    if (left.identifier is! hir.PropertyIdentifier &&
        left.identifier is! hir.LocalPropertyIdentifier) {
      return Error([
        CompilerError.invalidExpressionType('This is not a property.'),
      ]);
    }

    final isMutable = left.identifier.isMutableOrNull;
    if (!isMutable) {
      return Error([
        CompilerError.assignmentToImmutable(
          "Can't assign to an immutable property.",
          location: ErrorLocation(resourceId, expression.operatorToken.span),
        ),
      ]);
    }

    final rightExpression =
        innerExpressionContext(expressionType: Some(left.type))
            .lowerUnambiguous(expression.rightOperand);
    if (rightExpression is Error) return Error(rightExpression.error);
    final right = rightExpression.value;

    return Ok([hir.AssignmentExpression(getId(expression), left, right)]);
  }
}
