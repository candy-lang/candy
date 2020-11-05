import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart' hide Token;
import 'package:petitparser/petitparser.dart';

import 'analysis_server.dart';
import 'generated/lsp_protocol/protocol_generated.dart';

bool isCandyDocument(String uri) => uri.endsWith('.candy');

extension PositionToOffset on Position {
  int toOffset(AnalysisServer server, ResourceId resourceId) {
    final context = QueryContext(server.queryConfig.createContext());
    final source = server.resourceProvider.getContent(context, resourceId);

    final lineOffset = line == 0
        ? 0
        : Token.newlineParser().token().matchesSkipping(source)[line - 1].stop;
    return lineOffset + character;
  }
}

extension OffsetToPosition on int {
  Position toPosition(AnalysisServer server, ResourceId resourceId) {
    final context = QueryContext(server.queryConfig.createContext());
    final source = server.resourceProvider.getContent(context, resourceId);

    var line = 0;
    var column = 0;
    for (final lineToken
        in Token.newlineParser().token().matchesSkipping(source)) {
      if (this < lineToken.stop) return Position(line, this - column);

      line++;
      column = lineToken.stop;
    }
    return Position(line, this - column);
  }
}

extension SourceSpanToRange on SourceSpan {
  Range toRange(AnalysisServer server, ResourceId resourceId) {
    return Range(
      start.toPosition(server, resourceId),
      end.toPosition(server, resourceId),
    );
  }
}

extension ErrorLocationConversion on ErrorLocation {
  Location toLocation(AnalysisServer server) =>
      Location(server.resourceIdToFileUri(resourceId), toRange(server));
  Range toRange(AnalysisServer server) {
    return Range(
      span.start.toPosition(server, resourceId),
      span.end.toPosition(server, resourceId),
    );
  }
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
