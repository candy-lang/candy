import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import 'handlers.dart';
import 'states.dart';

/// Helper for reading client dynamic registrations which may be ommitted by the
/// client.
class ClientDynamicRegistrations {
  ClientDynamicRegistrations(this._capabilities);

  final ClientCapabilities _capabilities;
}

class InitializeMessageHandler
    extends MessageHandler<InitializeParams, InitializeResult> {
  InitializeMessageHandler(AnalysisServer server) : super(server);

  @override
  Method get handlesMessage => Method.initialize;

  @override
  LspJsonHandler<InitializeParams> get jsonHandler =>
      InitializeParams.jsonHandler;

  @override
  ErrorOr<InitializeResult> handle(
    InitializeParams params,
    CancellationToken token,
  ) {
    server
      ..handleClientConnection(
        params.capabilities,
        params.initializationOptions,
      )
      ..messageHandler = InitializingStateMessageHandler(server);

    final dynamicRegistrations =
        ClientDynamicRegistrations(params.capabilities);

    // When adding new capabilities to the server that may apply to specific file
    // types, it's important to update
    // [IntializedMessageHandler._performDynamicRegistration()] to notify
    // supporting clients of this. This avoids clients needing to hard-code the
    // list of what files types we support (and allows them to avoid sending
    // requests where we have only partial support for some types).
    server.capabilities = ServerCapabilities(
      dynamicRegistrations.textSync
          ? null
          : Either2<TextDocumentSyncOptions, num>.t1(TextDocumentSyncOptions(
              // The open/close and sync kind flags are registered dynamically if the
              // client supports them, so these static registrations are based on whether
              // the client supports dynamic registration.
              true,
              TextDocumentSyncKind.Incremental,
              false,
              false,
              null,
            )),
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      dynamicRegistrations.folding ? null : true, // foldingRangeProvider
      null, // declarationProvider
      null,
      null,
      null,
    );

    return success(InitializeResult(server.capabilities));
  }
}
