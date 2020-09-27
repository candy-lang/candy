import '../analysis_server.dart';
import '../error_codes.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import 'handlers.dart';

class TextDocumentOpenHandler
    extends MessageHandler<DidOpenTextDocumentParams, void> {
  TextDocumentOpenHandler(AnalysisServer server) : super(server);

  DateTime lastSentAnalyzeOpenFilesWarnings;

  @override
  Method get handlesMessage => Method.textDocument_didOpen;
  @override
  LspJsonHandler<DidOpenTextDocumentParams> get jsonHandler =>
      DidOpenTextDocumentParams.jsonHandler;

  @override
  ErrorOr<void> handle(
    DidOpenTextDocumentParams params,
    CancellationToken token,
  ) {
    final resourceId = server.fileUriToResourceId(params.textDocument.uri);
    server.resourceProvider.addOverlay(resourceId, params.textDocument.text);
    server.onFileChanged();
    return success();
  }
}

class TextDocumentChangeHandler
    extends MessageHandler<DidChangeTextDocumentParams, void> {
  TextDocumentChangeHandler(AnalysisServer server) : super(server);

  @override
  Method get handlesMessage => Method.textDocument_didChange;
  @override
  LspJsonHandler<DidChangeTextDocumentParams> get jsonHandler =>
      DidChangeTextDocumentParams.jsonHandler;

  @override
  ErrorOr<void> handle(
    DidChangeTextDocumentParams params,
    CancellationToken token,
  ) {
    final resourceId = server.fileUriToResourceId(params.textDocument.uri);
    if (!server.resourceProvider.hasOverlay(resourceId)) {
      return error(
        ServerErrorCodes.ClientServerInconsistentState,
        'Unable to edit document because the file was not previously opened: $resourceId',
        null,
      );
    }

    final changeError = server.resourceProvider
        .updateOverlay(resourceId, params.contentChanges);
    server.onFileChanged();
    return changeError ?? success();
  }
}

class TextDocumentCloseHandler
    extends MessageHandler<DidCloseTextDocumentParams, void> {
  TextDocumentCloseHandler(AnalysisServer server) : super(server);

  @override
  Method get handlesMessage => Method.textDocument_didClose;
  @override
  LspJsonHandler<DidCloseTextDocumentParams> get jsonHandler =>
      DidCloseTextDocumentParams.jsonHandler;

  @override
  ErrorOr<void> handle(
    DidCloseTextDocumentParams params,
    CancellationToken token,
  ) {
    final resourceId = server.fileUriToResourceId(params.textDocument.uri);
    server.resourceProvider.removeOverlay(resourceId);
    return success();
  }
}
