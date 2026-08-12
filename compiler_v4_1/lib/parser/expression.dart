import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import '../ast.dart';
import '../position.dart';
import 'list.dart';
import 'literal.dart';
import 'parser.dart';
import 'whitespace.dart';
import 'word.dart';

// pub fn expression(parser: Parser) -> Option<(Parser, AstExpression)> {
//     // If we start the call list with `if … else …`, the formatting looks weird.
//     // Hence, we start with a single `None`.
//     let start_offset = parser.offset();
//     let (mut parser, kind) = None
//         .or_else(|| {
//             raw_identifier(parser).map(|(parser, identifier)| {
//                 (
//                     parser,
//                     AstExpressionKind::Identifier(AstIdentifier { identifier }),
//                 )
//             })
//         })
//         .or_else(|| int(parser).map(|(parser, it)| (parser, AstExpressionKind::Int(it))))
//         .or_else(|| text(parser).map(|(parser, it)| (parser, AstExpressionKind::Text(it))))
//         .or_else(|| lambda(parser).map(|(parser, it)| (parser, AstExpressionKind::Lambda(it))))
//         .or_else(|| {
//             parenthesized(parser).map(|(parser, it)| (parser, AstExpressionKind::Parenthesized(it)))
//         })
//         .or_else(|| body(parser).map(|(parser, it)| (parser, AstExpressionKind::Body(it))))
//         .or_else(|| switch(parser).map(|(parser, it)| (parser, AstExpressionKind::Switch(it))))?;
//     let span = start_offset..parser.offset();
//     let mut result = AstExpression { span, kind };

//     loop {
//         fn parse_suffix<'a>(
//             parser: &mut Parser<'a>,
//             result: &mut AstExpression,
//             parse: fn(Parser<'a>, &mut AstExpression) -> Option<Parser<'a>>,
//         ) -> bool {
//             parse(*parser, result).is_some_and(|new_parser| {
//                 *parser = new_parser;
//                 true
//             })
//         }

//         let mut did_make_progress = false;
//         did_make_progress |= parse_suffix(&mut parser, &mut result, expression_suffix_navigation);
//         did_make_progress |= parse_suffix(&mut parser, &mut result, expression_suffix_call);
//         if !did_make_progress {
//             break;
//         }
//     }
//     Some((parser, result))
// }

(Parser, AstExpression)? expression(Parser parser) {
  final startOffset = parser.offset;

  final kindResult =
      rawIdentifier(
        parser,
      )?.mapValue((it) => AstExpressionKind.identifier(AstIdentifier(it))) ??
      int(parser)?.mapValue(AstExpressionKind.int);
  // TODO: other expression kinds
  if (kindResult == null) return null;

  final span = startOffset.rangeTo(kindResult.$1.offset);
  var result = AstExpression(
    span: SourceSpan(parser.file, span),
    kind: kindResult.$2,
  );

  while (true) {
    bool parseSuffix(
      (Parser, AstExpression)? Function(Parser, AstExpression) parse,
    ) {
      final newResult = parse(parser, result);
      if (newResult == null) return false;

      parser = newResult.$1;
      result = newResult.$2;
      return true;
    }

    if (!parseSuffix(expressionSuffixNavigation) &&
        !parseSuffix(expressionSuffixCall)) {
      break;
    }
  }
  return (kindResult.$1, result);
}

(Parser, AstInt)? int(Parser parser) {
  final wordResult = word(parser);
  if (wordResult == null) return null;

  if (!RegExp(r'\d').hasMatch(wordResult.$2.string.characters.first)) {
    return null;
  }

  return (
    parser,
    AstInt(
      value: RegExp(r'^\d+$').hasMatch(wordResult.$2.string)
          ? AstResult.ok(wordResult.$2.string.toInt())
          : AstResult.error(
              null,
              wordResult.$1.addErrorFrom(
                parser,
                'This integer contains characters that are not digits.',
              ),
            ),
      string: wordResult.$2,
    ),
  );
}

(Parser, AstParenthesized)? parenthesized(Parser parser) {
  final openingParenthesisResult = openingParenthesis(
    parser,
  )?.andTrailingWhitespace();
  if (openingParenthesisResult == null) return null;

  final innerResult = expression(openingParenthesisResult)
      .andTrailingWhitespace()
      .unwrapOrError(
        openingParenthesisResult,
        'This parenthesized expression is missing a closing parenthesis.',
      );

  final closingParenthesisResult = closingParenthesis(innerResult.$1)
      .unwrapOrError(
        openingParenthesisResult,
        'This parenthesized expression is missing a closing parenthesis.',
      );

  return (
    closingParenthesisResult.$1,
    AstParenthesized(
      inner: innerResult.$2,
      closingParenthesisError: closingParenthesisResult.$2,
    ),
  );
}

(Parser, AstLambda)? lambda(Parser parser) {
  final parametersResult = parameters(parser).andTrailingWhitespace();
  if (parametersResult == null) return null;

  final bodyResult = body(parametersResult.$1);
  if (bodyResult == null) return null;

  return (
    bodyResult.$1,
    AstLambda(parameters: parametersResult.$2, body: bodyResult.$2),
  );
}
// pub fn body<'a>(parser: Parser) -> Option<(Parser, AstBody)> {
//     let parser = opening_curly_brace(parser)?.and_trailing_whitespace();

//     let (parser, statements) = list_of(parser, statement);

//     let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
//         .unwrap_or_ast_error(parser, "This body is missing a closing curly brace.");

//     Some((
//         parser,
//         AstBody {
//             statements,
//             closing_curly_brace_error,
//         },
//     ))
// }
// pub fn statement(parser: Parser) -> Option<(Parser, AstStatement)> {
//     assignment(parser)
//         .map(|(parser, it)| (parser, AstStatement::Assignment(it)))
//         .or_else(|| expression(parser).map(|(parser, it)| (parser, AstStatement::Expression(it))))
// }

(Parser, AstBody)? body(Parser parser) {
  final openingCurlyBraceResult = openingCurlyBrace(
    parser,
  )?.andTrailingWhitespace();
  if (openingCurlyBraceResult == null) return null;

  final statementsResult = listOf(
    openingCurlyBraceResult,
    statement,
  ).andTrailingWhitespace();

  final closingCurlyBraceResult = closingCurlyBrace(statementsResult.$1)
      .unwrapOrError(
        openingCurlyBraceResult,
        'This body is missing a closing curly brace.',
      );

  return (
    closingCurlyBraceResult.$1,
    AstBody(
      statements: statementsResult.$2,
      closingCurlyBraceError: closingCurlyBraceResult.$2,
    ),
  );
}

(Parser, AstStatement)? statement(Parser parser) {
  return assignment(parser)?.mapValue(AstStatement.assignment) ??
      expression(parser)?.mapValue(AstStatement.expression);
}

// fn switch<'a>(parser: Parser) -> Option<(Parser, AstSwitch)> {
//     let parser = switch_keyword(parser)?.and_trailing_whitespace();

//     let (parser, value) = expression(parser)
//         .and_trailing_whitespace()
//         .unwrap_or_ast_error(
//             parser,
//             "This switch is missing an expression to switch over.",
//         );

//     let (mut parser, opening_curly_brace_error) = opening_curly_brace(parser)
//         .and_trailing_whitespace()
//         .unwrap_or_ast_error(parser, "This switch is missing an opening curly brace.");

//     let mut cases: Vec<AstSwitchCase> = vec![];
//     let mut parser_for_missing_comma_error: Option<Parser> = None;
//     while let Some((new_parser, case, new_parser_for_missing_comma_error)) =
//         switch_case(parser.and_trailing_whitespace())
//     {
//         if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
//             cases.last_mut().unwrap().comma_error = Some(
//                 parser_for_missing_comma_error
//                     .error_at_current_offset("This case is missing a comma."),
//             );
//         }

//         parser = new_parser;
//         cases.push(case);
//         parser_for_missing_comma_error = new_parser_for_missing_comma_error;
//     }
//     let parser = parser.and_trailing_whitespace();

//     let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
//         .unwrap_or_ast_error(parser, "This switch is missing a closing curly brace.");

//     Some((
//         parser,
//         AstSwitch {
//             value: value.map(Box::new),
//             opening_curly_brace_error,
//             cases,
//             closing_curly_brace_error,
//         },
//     ))
// }
// fn switch_case<'a>(parser: Parser) -> Option<(Parser, AstSwitchCase, Option<Parser>)> {
//     let (parser, variant) = raw_identifier(parser)?.and_trailing_whitespace();

//     let (parser, value_name) = opening_parenthesis(parser)
//         .and_trailing_whitespace()
//         .map_or((parser, None), |parser| {
//             let (parser, value_name) = raw_identifier(parser)
//                 .and_trailing_whitespace()
//                 .unwrap_or_ast_error_result(parser, "Switch case is missing a value name.");
//             let (parser, closing_parenthesis_error) = closing_parenthesis(parser)
//                 .and_trailing_whitespace()
//                 .unwrap_or_ast_error(
//                     parser,
//                     "Switch case is missing a closing parenthesis after the value name.",
//                 );
//             (parser, Some((value_name, closing_parenthesis_error)))
//         });

//     let (parser, arrow_error) = arrow(parser)
//         .and_trailing_whitespace()
//         .unwrap_or_ast_error(parser, "Switch case is missing an arrow.");

//     let (parser, expression) = expression(parser)
//         .and_trailing_whitespace()
//         .unwrap_or_ast_error(parser, "This switch case is missing an expression.");

//     let (parser, parser_for_missing_comma_error) =
//         comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

//     Some((
//         parser,
//         AstSwitchCase {
//             variant,
//             value_name,
//             arrow_error,
//             expression,
//             comma_error: None,
//         },
//         parser_for_missing_comma_error,
//     ))
// }

(Parser, AstSwitchCase, Parser?)? switchCase(Parser parser) {
  final variantResult = rawIdentifier(parser)?.andTrailingWhitespace();
  if (variantResult == null) return null;
  parser = variantResult.$1;
  final variant = variantResult.$2;

  final valueNameResult =
      openingParenthesis(parser).andTrailingWhitespace()?.let((it) {
        final valueNameResult = rawIdentifier(it)
            .andTrailingWhitespace()
            .unwrapOrErrorResult(it, 'Switch case is missing a value name.');
        var parser = valueNameResult.$1;
        final valueName = valueNameResult.$2;

        final closingParenthesisResult = closingParenthesis(parser)
            .andTrailingWhitespace()
            .unwrapOrError(
              it,
              'Switch case is missing a closing parenthesis after the value name.',
            );
        parser = closingParenthesisResult.$1;
        final closingParenthesisError = closingParenthesisResult.$2;

        return (parser, (valueName, closingParenthesisError));
      }) ??
      (parser, null);
  parser = valueNameResult.$1;
}

(Parser, AstExpression)? expressionSuffixNavigation(
  Parser parser,
  AstExpression current,
) {
  final startOffset = parser.offset;
  parser = whitespace(parser) ?? parser;

  final dotResult = dot(parser)?.andTrailingWhitespace();
  if (dotResult == null) return null;
  parser = dotResult;

  final keyResult = rawIdentifier(
    parser,
  ).unwrapOrErrorResult(parser, 'This navigation is missing a key.');
  parser = keyResult.$1;
  final key = keyResult.$2;

  return (
    parser,
    AstExpression(
      span: SourceSpan(parser.file, startOffset.rangeTo(parser.offset)),
      kind: .navigation(AstNavigation(receiver: current, key: key)),
    ),
  );
}

(Parser, AstExpression)? expressionSuffixCall(
  Parser parser,
  AstExpression current,
) {
  final startOffset = parser.offset;
  parser = whitespace(parser) ?? parser;

  final typeArgumentsResult = typeArguments(parser).optional(parser);
  parser = typeArgumentsResult.$1;

  final AstResult<AstArguments>? argumentsResult;
  if (arguments(parser) case final argumentsResultInner?) {
    parser = argumentsResultInner.$1;
    argumentsResult = AstResult.ok(argumentsResultInner.$2);
  } else {
    if (typeArgumentsResult.$2 != null) return null;

    argumentsResult = AstResult.error(
      null,
      parser.addErrorAtCurrentOffset('This call is missing arguments.'),
    );
  }

  return (
    parser,
    AstExpression(
      span: SourceSpan(parser.file, startOffset.rangeTo(parser.offset)),
      kind: .call(
        AstCall(
          receiver: current,
          typeArguments: typeArgumentsResult.$1,
          arguments: argumentsResult,
        ),
      ),
    ),
  );
}

(Parser, AstArguments)? arguments(Parser parser) {
  final openingParenthesisStart = parser.offset;
  final openingParenthesisResult = openingParenthesis(
    parser,
  )?.andTrailingWhitespace();
  if (openingParenthesisResult == null) return null;
  parser = openingParenthesisResult;

  final openingParenthesisSpan = openingParenthesisStart.rangeTo(
    openingParenthesisResult.offset,
  );

  final arguments = <AstArgument>[];
  Parser? parserForMissingCommaError;
  while (true) {
    final argumentResult = argument(parser.andTrailingWhitespace());
    if (argumentResult == null) break;

    if (parserForMissingCommaError != null) {
      arguments.last = arguments.last.copyWith(
        commaError: parserForMissingCommaError.addErrorAtCurrentOffset(
          'This type argument is missing a comma.',
        ),
      );
    }

    parser = argumentResult.$1;
    arguments.add(argumentResult.$2);
    parserForMissingCommaError = argumentResult.$3;
  }
  parser = parser.andTrailingWhitespace();

  final closingParenthesisResult = closingParenthesis(parser).unwrapOrError(
    openingParenthesisResult,
    'This call is missing a closing parenthesis.',
  );
  parser = closingParenthesisResult.$1;
  final closingParenthesisError = closingParenthesisResult.$2;

  return (
    parser,
    AstArguments(
      openingParenthesisSpan: SourceSpan(parser.file, openingParenthesisSpan),
      arguments: arguments,
      closingParenthesisError: closingParenthesisError,
    ),
  );
}

(Parser, AstArgument, Parser?)? argument(Parser parser) {
  final startOffset = parser.offset;
  final valueResult = expression(parser)?.andTrailingWhitespace();
  if (valueResult == null) return null;

  final (parserAfterComma, parserForMissingCommaError) =
      comma(valueResult.$1)?.let((parser) => (parser, null)) ??
      ((valueResult.$1, valueResult.$1));

  return (
    parserAfterComma,
    AstArgument(
      value: valueResult.$2,
      commaError: null,
      span: SourceSpan(parser.file, startOffset.rangeTo(parser.offset)),
    ),
    parserForMissingCommaError,
  );
}
