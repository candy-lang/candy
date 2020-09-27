import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart' hide Token;
import 'package:petitparser/petitparser.dart';

import 'analysis_server.dart';
import 'generated/lsp_protocol/protocol_generated.dart';

bool isCandyDocument(String uri) => uri.endsWith('.candy');

extension SourceSpanToRange on SourceSpan {
  Range toRange(AnalysisServer server, ResourceId resourceId) {
    final source = server.resourceProvider.getContent(resourceId);
    return Range(positionOf(source, start), positionOf(source, end));
  }
}

Position positionOf(String buffer, int offset) {
  var line = 0;
  var column = 0;
  for (final token in Token.newlineParser().token().matchesSkipping(buffer)) {
    if (offset < token.stop) return Position(line, offset - column);

    line++;
    column = token.stop;
  }
  return Position(line, offset - column);
}

/// Combines the [Object.hashCode] values of an arbitrary number of objects
/// from an [Iterable] into one value. This function will return the same
/// value if given `null` as if given an empty list.
// Borrowed from dart:ui.
int hashList(Iterable<Object> arguments) {
  var result = 0;
  if (arguments != null) {
    for (final argument in arguments) {
      var hash = result;
      hash = 0x1fffffff & (hash + argument.hashCode);
      hash = 0x1fffffff & (hash + ((0x0007ffff & hash) << 10));
      result = hash ^ (hash >> 6);
    }
  }
  result = 0x1fffffff & (result + ((0x03ffffff & result) << 3));
  result = result ^ (result >> 11);
  return 0x1fffffff & (result + ((0x00003fff & result) << 15));
}
