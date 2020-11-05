import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import 'code_action.dart';
import 'handlers.dart';
import 'states.dart';

class IntializedMessageHandler extends MessageHandler<InitializedParams, void> {
  IntializedMessageHandler(AnalysisServer server) : super(server);

  @override
  Method get handlesMessage => Method.initialized;
  @override
  LspJsonHandler<InitializedParams> get jsonHandler =>
      InitializedParams.jsonHandler;

  @override
  ErrorOr<void> handle(InitializedParams params, CancellationToken token) {
    server.messageHandler = InitializedStateMessageHandler(server);
    _performDynamicRegistration();
    return success();
  }

  /// If the client supports dynamic registrations we can tell it what methods
  /// we support for which documents. For example, this allows us to ask for
  /// file edits for .dart as well as pubspec.yaml but only get hover/completion
  /// calls for .dart. This functionality may not be supported by the client, in
  /// which case they will use the ServerCapabilities to know which methods we
  /// support and it will be up to them to decide which file types they will
  /// send requests for.
  Future<void> _performDynamicRegistration() async {
    final candyFiles = DocumentFilter('candy', 'file', null);

    var _lastRegistrationId = 1;
    final registrations = <Registration>[];
    // ignore:avoid_positional_boolean_parameters
    void register(bool condition, Method method, [ToJsonable options]) {
      if (condition == true) {
        registrations.add(Registration(
          (_lastRegistrationId++).toString(),
          method.toJson() as String,
          options,
        ));
      }
    }

    final textCapabilities = server.clientCapabilities?.textDocument;

    register(
      textCapabilities?.synchronization?.dynamicRegistration,
      Method.textDocument_didOpen,
      TextDocumentRegistrationOptions([candyFiles]),
    );
    register(
      textCapabilities?.synchronization?.dynamicRegistration,
      Method.textDocument_didClose,
      TextDocumentRegistrationOptions([candyFiles]),
    );
    register(
      textCapabilities?.synchronization?.dynamicRegistration,
      Method.textDocument_didChange,
      TextDocumentChangeRegistrationOptions(
        TextDocumentSyncKind.Incremental,
        [candyFiles],
      ),
    );
    register(
      textCapabilities?.hover?.dynamicRegistration,
      Method.textDocument_hover,
      TextDocumentRegistrationOptions([candyFiles]),
    );
    register(
      server.clientCapabilities?.textDocument?.codeAction?.dynamicRegistration,
      Method.textDocument_codeAction,
      CodeActionRegistrationOptions(
        [candyFiles],
        CandyCodeActionKind.serverSupportedKinds,
      ),
    );
    register(
      textCapabilities?.foldingRange?.dynamicRegistration,
      Method.textDocument_foldingRange,
      TextDocumentRegistrationOptions([candyFiles]),
    );

    // Only send the registration request if we have at least one (since
    // otherwise we don't know that the client supports registerCapability).
    if (registrations.isNotEmpty) {
      final registrationResponse = await server.sendRequest(
        Method.client_registerCapability,
        RegistrationParams(registrations),
      );

      if (registrationResponse.error != null) {
        server.logErrorToClient(
          'Failed to register capabilities with client: '
          '(${registrationResponse.error.code}) '
          '${registrationResponse.error.message}',
        );
      }
    }
  }
}
