import 'dart:io';

import 'package:analyzer/source/line_info.dart';
import 'package:compiler/compiler.dart';

import 'error_codes.dart';
import 'generated/lsp_protocol/protocol_generated.dart';
import 'generated/lsp_protocol/protocol_special.dart';

class OverlayResourceProvider extends ResourceProvider {
  OverlayResourceProvider(this.baseProvider) : assert(baseProvider != null);

  final ResourceProvider baseProvider;

  @override
  Directory get candyDirectory => baseProvider.candyDirectory;
  @override
  Directory get projectDirectory => baseProvider.projectDirectory;

  final _overlays = <ResourceId, String>{};

  @override
  List<ResourceId> getAllFileResourceIds(
    QueryContext context,
    PackageId packageId,
  ) {
    // TODO: check overlays
    return baseProvider.getAllFileResourceIds(context, packageId);
  }

  @override
  bool fileExists(QueryContext context, ResourceId id) =>
      _overlays.containsKey(id) || baseProvider.fileExists(context, id);
  @override
  bool directoryExists(QueryContext context, ResourceId id) =>
      baseProvider.directoryExists(context, id);

  @override
  String getContent(QueryContext context, ResourceId id) =>
      _overlays[id] ?? baseProvider.getContent(context, id);

  void addOverlay(ResourceId id, String content) => _overlays[id] = content;
  ErrorOr<void> updateOverlay(
    ResourceId id,
    List<TextDocumentContentChangeEvent> changes,
  ) {
    var newContent = _overlays[id];
    assert(newContent != null);

    for (final change in changes) {
      if (change.range == null && change.rangeLength == null) {
        newContent = change.text;
      } else {
        final lines = LineInfo.fromContent(newContent);
        final offsetStart = _toOffset(lines, change.range.start);
        if (offsetStart.isError) return ErrorOr.error(offsetStart.error);
        final offsetEnd = _toOffset(lines, change.range.end);
        if (offsetEnd.isError) return ErrorOr.error(offsetEnd.error);

        newContent = newContent.replaceRange(
          offsetStart.result,
          offsetEnd.result,
          change.text,
        );
      }
    }
    _overlays[id] = newContent;
    return null;
  }

  void removeOverlay(ResourceId id) => _overlays.remove(id);
  bool hasOverlay(ResourceId id) => _overlays.containsKey(id);
}

ErrorOr<int> _toOffset(LineInfo lineInfo, Position pos) {
  if (pos.line > lineInfo.lineCount) {
    return ErrorOr<int>.error(ResponseError(
      ServerErrorCodes.ClientServerInconsistentState,
      'Invalid line number',
      pos.line.toString(),
    ));
  }
  return ErrorOr<int>.success(
    lineInfo.getOffsetOfLine(pos.line) + pos.character,
  );
}
