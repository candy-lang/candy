// Copyright (c) 2018, the Dart project authors. Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import 'handlers.dart';

/// A [MessageHandler] that rejects specific types of messages with a given
/// error code/message.
class RejectMessageHandler extends MessageHandler<Object, void> {
  RejectMessageHandler(
    AnalysisServer server,
    this.handlesMessage,
    this.errorCode,
    this.errorMessage,
  ) : super(server);

  @override
  final Method handlesMessage;
  final ErrorCodes errorCode;
  final String errorMessage;

  @override
  LspJsonHandler<void> get jsonHandler => NullJsonHandler;

  @override
  ErrorOr<void> handle(void _, CancellationToken token) =>
      error(errorCode, errorMessage, null);
}
