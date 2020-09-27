// Copyright (c) 2018, the Dart project authors. Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

import 'dart:async';

import '../analysis_server.dart';
import '../error_codes.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import 'exit.dart';
import 'folding.dart';
import 'handlers.dart';
import 'initialize.dart';
import 'initialized.dart';
import 'shutdown.dart';
import 'text_synchronization.dart';

class UninitializedStateMessageHandler extends ServerStateMessageHandler {
  UninitializedStateMessageHandler(AnalysisServer server) : super(server) {
    registerHandler(InitializeMessageHandler(server));
    registerHandler(ShutdownMessageHandler(server));
    registerHandler(ExitMessageHandler(server));
  }

  @override
  FutureOr<ErrorOr<Object>> handleUnknownMessage(IncomingMessage message) {
    // Silently drop non-requests.
    if (message is! RequestMessage) return success();

    return error(
      ErrorCodes.ServerNotInitialized,
      'Unable to handle ${message.method} before client has sent initialize '
      'request',
    );
  }
}

class InitializingStateMessageHandler extends ServerStateMessageHandler {
  InitializingStateMessageHandler(AnalysisServer server) : super(server) {
    reject(
      Method.initialize,
      ServerErrorCodes.ServerAlreadyInitialized,
      'Server already initialized',
    );
    registerHandler(ShutdownMessageHandler(server));
    registerHandler(ExitMessageHandler(server));
    registerHandler(IntializedMessageHandler(server));
  }

  @override
  ErrorOr<void> handleUnknownMessage(IncomingMessage message) {
    // Silently drop non-requests.
    if (message is! RequestMessage) return success();

    return error(
      ErrorCodes.ServerNotInitialized,
      'Unable to handle ${message.method} before the server is initialized and '
      'the client has sent the initialized notification',
    );
  }
}

class InitializedStateMessageHandler extends ServerStateMessageHandler {
  InitializedStateMessageHandler(AnalysisServer server) : super(server) {
    reject(
      Method.initialize,
      ServerErrorCodes.ServerAlreadyInitialized,
      'Server already initialized',
    );
    reject(
      Method.initialized,
      ServerErrorCodes.ServerAlreadyInitialized,
      'Server already initialized',
    );
    registerHandler(ShutdownMessageHandler(server));
    registerHandler(ExitMessageHandler(server));
    registerHandler(TextDocumentOpenHandler(server));
    registerHandler(TextDocumentChangeHandler(server));
    registerHandler(TextDocumentCloseHandler(server));
    registerHandler(FoldingHandler(server));
  }
}

class ShuttingDownStateMessageHandler extends ServerStateMessageHandler {
  ShuttingDownStateMessageHandler(AnalysisServer server) : super(server) {
    registerHandler(ExitMessageHandler(server, clientDidCallShutdown: true));
  }

  @override
  FutureOr<ErrorOr<Object>> handleUnknownMessage(IncomingMessage message) {
    // Silently drop non-requests.
    if (message is! RequestMessage) return success();

    return error(
      ErrorCodes.InvalidRequest,
      'Unable to handle ${message.method} after shutdown request',
    );
  }
}

/// The server moves to this state when a critical unrecoverrable error (for
/// example, inconsistent document state between server/client) occurs and will
/// reject all messages.
class FailureStateMessageHandler extends ServerStateMessageHandler {
  FailureStateMessageHandler(AnalysisServer server) : super(server);

  @override
  FutureOr<ErrorOr<Object>> handleUnknownMessage(IncomingMessage message) {
    return error(
      ErrorCodes.InternalError,
      'An unrecoverable error occurred and the server cannot process messages',
      null,
    );
  }
}
