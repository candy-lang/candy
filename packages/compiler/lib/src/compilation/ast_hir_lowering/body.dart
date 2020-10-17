import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' as ast;

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
import 'type.dart';

final getBody = Query<DeclarationId, List<hir.Statement>>(
  'getBody',
  provider: (context, declarationId) =>
      lowerBodyAstToHir(context, declarationId).first,
);
final getBodyAstToHirIds = Query<DeclarationId, BodyAstToHirIds>(
  'getBodyAstToHirIds',
  provider: (context, declarationId) =>
      lowerBodyAstToHir(context, declarationId).second,
);
final Query<DeclarationId, Tuple2<List<hir.Statement>, BodyAstToHirIds>>
    lowerBodyAstToHir =
    Query<DeclarationId, Tuple2<List<hir.Statement>, BodyAstToHirIds>>(
  'lowerBodyAstToHir',
  provider: (context, declarationId) {
    if (declarationId.isFunction) {
      final functionAst = getFunctionDeclarationAst(context, declarationId);

      final localContext = _LocalContext.forFunction(
        context,
        declarationId,
        functionAst,
      );
      final statements =
          functionAst.body.statements.map<hir.Statement>((statement) {
        if (statement is ast.Expression) {
          final result = localContext.lowerToUnambiguous(statement);
          // ignore: only_throw_errors, Iterables of errors are also handled.
          if (result is Error) throw result.error;

          return hir.Statement.expression(
            localContext.getId(statement),
            result.value,
          );
        } else {
          throw CompilerError.unsupportedFeature(
            'Unsupported statement.',
            location: ErrorLocation(declarationId.resourceId, statement.span),
          );
        }
      }).toList();
      return Tuple2(statements, localContext.idMap);
    } else if (declarationId.isProperty) {
      final propertyAst = getPropertyDeclarationAst(context, declarationId);
      if (propertyAst.initializer == null) {
        throw CompilerError.internalError(
          '`lowerBodyAstToHir` called on a property without an initializer.',
          location:
              ErrorLocation(declarationId.resourceId, propertyAst.name.span),
        );
      }

      var type = Option<hir.CandyType>.none();
      if (propertyAst.type != null) {
        final moduleId = declarationIdToModuleId(context, declarationId);
        type = Option.some(
          astTypeToHirType(context, Tuple2(moduleId, propertyAst.type)),
        );
      }
      final localContext =
          _LocalContext.forProperty(context, declarationId, type);

      final result = localContext.lowerToUnambiguous(propertyAst.initializer);
      // ignore: only_throw_errors, Iterables of errors are also handled.
      if (result is Error) throw result.error;

      final statement = hir.Statement.expression(
        localContext.getId(propertyAst.initializer),
        result.value,
      );
      return Tuple2([statement], localContext.idMap);
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

typedef IdProvider = DeclarationLocalId Function(int astId);

class _LocalContext {
  _LocalContext._(
    this.queryContext,
    this.declarationId,
    this.expressionType,
    this.returnType, [
    this.localIdentifiers = const {},
  ])  : assert(queryContext != null),
        assert(declarationId != null),
        assert(expressionType != null),
        assert(returnType != null),
        assert(localIdentifiers != null);

  factory _LocalContext.forFunction(
    QueryContext context,
    DeclarationId declarationId,
    ast.FunctionDeclaration functionAst,
  ) {
    final moduleId = declarationIdToModuleId(context, declarationId);
    return _LocalContext._(
      context,
      declarationId,
      Option.none(),
      Option.some(
        functionAst.returnType == null
            ? CandyType.unit
            : astTypeToHirType(
                context, Tuple2(moduleId, functionAst.returnType)),
      ),
      <String, hir.Identifier>{
        for (final parameter in functionAst.valueParameters)
          parameter.name.name: hir.Identifier.parameter(
            parameter.name.name,
            0,
            astTypeToHirType(context, Tuple2(moduleId, parameter.type)),
          ),
      },
    );
  }
  factory _LocalContext.forProperty(
    QueryContext context,
    DeclarationId declarationId,
    Option<hir.CandyType> expressionType,
  ) {
    return _LocalContext._(
        context, declarationId, expressionType, Option.none());
  }

  final QueryContext queryContext;

  final DeclarationId declarationId;
  ResourceId get resourceId => declarationId.resourceId;

  var _nextId = 0;
  var idMap = BodyAstToHirIds();
  DeclarationLocalId getId(ast.Statement statement) {
    final id = DeclarationLocalId(declarationId, _nextId++);
    idMap = idMap.withMapping(statement.id, id);
    return id;
  }

  final Map<String, hir.Identifier> localIdentifiers;

  final Option<hir.CandyType> expressionType;
  bool isValidExpressionType(hir.CandyType type) {
    if (expressionType.isNone) return true;
    return hir.isAssignableTo(queryContext, Tuple2(type, expressionType.value));
  }

  final Option<hir.CandyType> returnType;
  bool isValidReturnType(hir.CandyType type) {
    if (returnType.isNone) return true;
    return hir.isAssignableTo(queryContext, Tuple2(type, returnType.value));
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> lower(
    ast.Expression expression, [
    Option<hir.CandyType> expressionType = const Option.none(),
  ]) {
    final innerContext = _LocalContext._(
      queryContext,
      declarationId,
      expressionType,
      returnType,
      localIdentifiers,
    );
    try {
      final result = innerContext._lowerExpression(expression);
      assert(returnType.isNone ||
          result is Error ||
          result.value.every((e) => hir.isAssignableTo(
              queryContext, Tuple2(e.type, returnType.value))));
      // if (!isEmpty)
      return result;
    } on _LoweringFailedException catch (e) {
      return Error(e.errors);
    }

    // Step 2: Retry with implicit cast of the whole expression
  }

  Result<List<hir.Expression>, List<ReportedCompilerError>> _lowerExpression(
    ast.Expression expression,
  ) {
    Result<List<hir.Expression>, List<ReportedCompilerError>> result;
    if (expression is ast.Literal) {
      result = _lowerLiteral(this, expression);
    } else if (expression is ast.StringLiteral) {
      result = _lowerStringLiteral(this, expression);
    } else if (expression is ast.Identifier) {
      result = _lowerIdentifier(this, expression);
    } else if (expression is ast.CallExpression) {
      result = _lowerCall(this, expression);
    } else if (expression is ast.ReturnExpression) {
      result = _lowerReturn(this, expression);
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported expression.',
        location: ErrorLocation(resourceId, expression.span),
      );
    }

    assert(result != null);
    return result;
  }

  List<hir.Expression> requireLowering(
    ast.Expression expression, [
    Option<hir.CandyType> expressionType = const Option.none(),
  ]) {
    final result = lower(expression, expressionType);
    if (result is Error) {
      throw _LoweringFailedException(result.error);
    }

    return result.value;
  }

  Result<hir.Expression, List<ReportedCompilerError>> lowerToUnambiguous(
    ast.Expression expression, [
    Option<hir.CandyType> expressionType = const Option.none(),
  ]) {
    final result = lower(expression, expressionType);
    if (result is Error) return Error(result.error);

    assert(result.value.isNotEmpty);
    if (result.value.length > 1) {
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

Result<List<T>, List<ReportedCompilerError>> _mergeResults<T>(
  Iterable<Result<List<T>, List<ReportedCompilerError>>> results,
) {
  final errors =
      results.whereType<Error<List<T>, List<ReportedCompilerError>>>();
  if (errors.isNotEmpty) errors.first;

  final oks = results.whereType<Ok<List<T>, List<ReportedCompilerError>>>();
  return Ok(oks.expand((ok) => ok.value).toList());
}

class _LoweringFailedException implements Exception {
  const _LoweringFailedException(this.errors) : assert(errors != null);

  final List<ReportedCompilerError> errors;
}

Result<List<hir.Expression>, List<ReportedCompilerError>> _lowerLiteral(
  _LocalContext context,
  ast.Literal<dynamic> expression,
) {
  final token = expression.value;
  hir.Literal literal;
  if (token is ast.BoolLiteralToken) {
    if (!context.isValidExpressionType(hir.CandyType.bool)) return Ok([]);
    literal = hir.Literal.boolean(token.value);
  } else if (token is ast.IntLiteralToken) {
    if (!context.isValidExpressionType(hir.CandyType.int)) return Ok([]);
    literal = hir.Literal.integer(token.value);
  } else {
    throw CompilerError.unsupportedFeature(
      'Unsupported literal.',
      location: ErrorLocation(context.resourceId, token.span),
    );
  }
  return Ok([hir.Expression.literal(context.getId(expression), literal)]);
}

Result<List<hir.Expression>, List<ReportedCompilerError>> _lowerStringLiteral(
  _LocalContext context,
  ast.StringLiteral expression,
) {
  final parts = expression.parts
      .map<Result<List<hir.StringLiteralPart>, List<ReportedCompilerError>>>(
          (part) {
    if (part is ast.LiteralStringLiteralPart) {
      return Ok([hir.StringLiteralPart.literal(part.value.value)]);
    } else if (part is ast.InterpolatedStringLiteralPart) {
      return context.lowerToUnambiguous(part.expression).mapValue(
          (expression) => [hir.StringLiteralPart.interpolated(expression)]);
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported String literal part.',
        location: ErrorLocation(context.resourceId, part.span),
      );
    }
  });
  return _mergeResults(parts).mapValue((parts) => [
        hir.Expression.literal(
          context.getId(expression),
          hir.StringLiteral(parts),
        ),
      ]);
}

Result<List<hir.Expression>, List<ReportedCompilerError>> _lowerIdentifier(
  _LocalContext context,
  ast.Identifier expression,
) {
  final identifier = expression.value.name;
  final localIdentifier = context.localIdentifiers[identifier];
  if (localIdentifier != null) {
    final result = hir.Expression.identifier(
      context.getId(expression),
      localIdentifier,
    );
    return Ok([result]);
  }

  if (context.expressionType.isNone && identifier == 'print') {
    return Ok([
      hir.Expression.identifier(
        context.getId(expression),
        hir.Identifier.property(
          hir.Expression.identifier(
            context.getId(expression),
            hir.Identifier.module(ModuleId(PackageId.this_, ['main'])),
          ),
          'print',
          hir.CandyType.function(
            parameterTypes: [hir.CandyType.any],
            returnType: hir.CandyType.unit,
          ),
        ),
      ),
    ]);
  }
  throw CompilerError.undefinedIdentifier(
    "Couldn't resolve identifier `$identifier`.",
    location: ErrorLocation(context.resourceId, expression.value.span),
  );
}

Result<List<hir.Expression>, List<ReportedCompilerError>> _lowerCall(
  _LocalContext context,
  ast.CallExpression expression,
) {
  final targetVariants = context.requireLowering(expression.target);

  final results = targetVariants.map((target) {
    if (target is hir.IdentifierExpression &&
        target.identifier is hir.PropertyIdentifier) {
      final identifier = target.identifier as hir.PropertyIdentifier;
      final declarationId = getPropertyIdentifierDeclarationId(
        context.queryContext,
        identifier,
      );
      if (declarationId.isFunction) {
        return _lowerFunctionCall(context, expression, target, identifier);
      }
    }

    throw CompilerError.unsupportedFeature(
      'Callable expressions are not yet supported.',
      location: ErrorLocation(context.resourceId, expression.span),
    );
  });
  return _mergeResults(results);
}

Result<List<hir.Expression>, List<ReportedCompilerError>> _lowerReturn(
  _LocalContext context,
  ast.ReturnExpression expression,
) {
  // The type of a `ReturnExpression` is `Never` and never is, by definition,
  // assignable to anything because it's a bottom type. So, we don't need to
  // check that.
  if (context.returnType == Option<CandyType>.none()) {
    throw CompilerError.returnNotInFunction(
        'The return expression is not in a function.');
  }
  return context
      .lowerToUnambiguous(expression.expression, context.returnType)
      .mapValue((hirExpression) => [
            hir.ReturnExpression(context.getId(expression), hirExpression),
          ]);
}

final getPropertyIdentifierDeclarationId =
    Query<hir.PropertyIdentifier, DeclarationId>(
  'getPropertyIdentifierDeclarationId',
  provider: (context, identifier) {
    final target = identifier.target;
    if (target is! hir.IdentifierExpression) {
      throw CompilerError.unsupportedFeature(
        'Properties of instances are not yet supported.',
      );
    }

    final targetIdentifier = (target as hir.IdentifierExpression).identifier;
    List<DeclarationId> innerDeclarationIds;
    if (targetIdentifier is hir.ModuleIdentifier) {
      innerDeclarationIds =
          getModuleDeclarationHir(context, targetIdentifier.id)
              .innerDeclarationIds;
    } else if (targetIdentifier is hir.TraitIdentifier) {
      innerDeclarationIds = getTraitDeclarationHir(context, targetIdentifier.id)
          .innerDeclarationIds;
      // } else if (targetIdentifier is hir.ClassIdentifier) {
      //   innerDeclarationIds = getClassDeclarationHir(context, targetIdentifier.id)
      //       .innerDeclarationIds;
    } else {
      assert(false);
      return null;
    }
    return innerDeclarationIds
        .firstWhere((id) => id.simplePath.last.nameOrNull == identifier.name);
  },
);

Result<List<hir.Expression>, List<ReportedCompilerError>> _lowerFunctionCall(
  _LocalContext context,
  ast.CallExpression expression,
  hir.IdentifierExpression target,
  hir.PropertyIdentifier targetIdentifier,
) {
  final functionDeclarationId = getPropertyIdentifierDeclarationId(
    context.queryContext,
    targetIdentifier,
  );
  assert(functionDeclarationId.isFunction);
  final functionHir =
      getFunctionDeclarationHir(context.queryContext, functionDeclarationId);
  if (!context.isValidExpressionType(functionHir.returnType)) return null;

  final errors = <ReportedCompilerError>[];

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
        location: ErrorLocation(context.resourceId, argument.span),
        relatedInformation: [
          for (final arg in outOfOrderNamedArguments)
            ErrorRelatedInformation(
              location: ErrorLocation(context.resourceId, arg.span),
              message: "A named argument that's not in the default order.",
            ),
        ],
      ));
      continue;
    }

    final parameter = parameters[i];
    if (argument.isPositional) {
      astArgumentMap[parameter.name] = argument;
    } else {
      assert(argument.name.name != null);
      astArgumentMap[argument.name.name] = argument;
      if (parameter.name != argument.name.name) {
        outOfOrderNamedArguments.add(argument);
      }
    }
  }
  if (errors.isNotEmpty) return Error(errors);

  final hirArgumentMap = <String, hir.Expression>{};
  for (final entry in astArgumentMap.entries) {
    final name = entry.key;
    final value = entry.value.expression;
    final lowered = context.lowerToUnambiguous(
      value,
      Option.some(parametersByName[name].type),
    );
    if (lowered is Error) {
      errors.addAll(lowered.error);
      continue;
    }

    hirArgumentMap[name] = lowered.value;
  }
  if (errors.isNotEmpty) return Error(errors);

  return Ok([
    hir.Expression.functionCall(
      context.getId(expression),
      target,
      hirArgumentMap,
    ),
  ]);
}
