import 'dart:io';

import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import 'code_action.dart';
import 'execute_command.dart';
import 'handlers.dart';
import 'states.dart';

/// Helper for reading client dynamic registrations which may be ommitted by the
/// client.
class ClientDynamicRegistrations {
  ClientDynamicRegistrations(this._capabilities);

  final ClientCapabilities _capabilities;

  bool get textSync =>
      _capabilities.textDocument?.synchronization?.dynamicRegistration ?? false;
  bool get hover =>
      _capabilities.textDocument?.hover?.dynamicRegistration ?? false;
  bool get definition =>
      _capabilities.textDocument?.definition?.dynamicRegistration ?? false;
  bool get folding =>
      _capabilities.textDocument?.foldingRange?.dynamicRegistration ?? false;
  bool get codeActions =>
      _capabilities.textDocument?.foldingRange?.dynamicRegistration ?? false;
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
        Directory.fromUri(Uri.parse(params.rootUri)),
      )
      ..messageHandler = InitializingStateMessageHandler(server);

    final codeActionLiteralSupport =
        params.capabilities.textDocument?.codeAction?.codeActionLiteralSupport;

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
      dynamicRegistrations.hover ? null : true, // hoverProvider
      null,
      null,
      dynamicRegistrations.definition ? null : true, // definitionProvider
      null,
      null,
      null,
      null,
      null,
      null,
      // "The `CodeActionOptions` return type is only valid if the client
      // signals code action literal support via the property
      // `textDocument.codeAction.codeActionLiteralSupport`."
      dynamicRegistrations.codeActions
          ? null
          : codeActionLiteralSupport != null
              ? Either2<bool, CodeActionOptions>.t2(
                  CodeActionOptions(CandyCodeActionKind.serverSupportedKinds),
                )
              : Either2<bool, CodeActionOptions>.t1(true),
      null,
      null,
      null,
      null,
      null,
      null,
      null,
      dynamicRegistrations.folding ? null : true, // foldingRangeProvider
      null, // declarationProvider
      ExecuteCommandOptions(Commands.all),
      null,
      null,
    );

    return success(InitializeResult(server.capabilities));
  }
}
