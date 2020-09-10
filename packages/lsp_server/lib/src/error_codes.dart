import 'generated/lsp_protocol/protocol_generated.dart';

abstract class ServerErrorCodes {
  // JSON-RPC reserves -32000 to -32099 for implementation-defined server-errors.
  static const ServerAlreadyStarted = ErrorCodes(-32000);
  static const UnhandledError = ErrorCodes(-32001);
  static const ServerAlreadyInitialized = ErrorCodes(-32002);
  static const InvalidFilePath = ErrorCodes(-32003);
  static const InvalidFileLineCol = ErrorCodes(-32004);
  static const UnknownCommand = ErrorCodes(-32005);
  static const InvalidCommandArguments = ErrorCodes(-32006);
  static const FileNotAnalyzed = ErrorCodes(-32007);
  static const FileHasErrors = ErrorCodes(-32008);
  static const ClientFailedToApplyEdit = ErrorCodes(-32009);
  static const RenameNotValid = ErrorCodes(-32010);
  static const RefactorFailed = ErrorCodes(-32011);

  /// An error raised when the server detects that the server and client are out
  /// of sync and cannot recover. For example if a textDocument/didChange notification
  /// has invalid offsets, suggesting the client and server have become out of sync
  /// and risk invalid modifications to a file.
  ///
  /// The server should detect this error being returned, log it, then exit.
  /// The client is expected to behave as suggested in the spec:
  ///
  ///  "If a client notices that a server exists unexpectedly it should try to
  ///   restart the server. However clients should be careful to not restart a
  ///   crashing server endlessly. VS Code for example doesn't restart a server
  ///   if it crashes 5 times in the last 180 seconds."
  static const ClientServerInconsistentState = ErrorCodes(-32010);
}
