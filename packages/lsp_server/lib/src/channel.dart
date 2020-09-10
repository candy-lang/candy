import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'analysis_server.dart';
import 'generated/lsp_protocol/protocol_generated.dart';
import 'generated/lsp_protocol/protocol_special.dart';
import 'json_parsing.dart';
import 'packet_transformer.dart';

/// The abstract class [LspServerCommunicationChannel] defines the behavior of
/// objects that allow an [AnalysisServer] to receive [RequestMessage]s and
/// [NotificationMessage]s and to return both [ResponseMessage]s and
/// [NotificationMessage]s.
abstract class LspServerCommunicationChannel {
  Future<void> get closed;

  /// Close the communication channel.
  void close();

  /// Listen to the channel for messages. If a message is received, invoke the
  /// [onMessage] function. If an error is encountered while trying to read from
  /// the socket, invoke the [onError] function. If the socket is closed by the
  /// client, invoke the [onDone] function.
  /// Only one listener is allowed per channel.
  void listen(
    void Function(Message message) onMessage, {
    Function onError,
    void Function() onDone,
  });

  /// Send the given [notification] to the client.
  void sendNotification(NotificationMessage notification);

  /// Send the given [request] to the client.
  void sendRequest(RequestMessage request);

  /// Send the given [response] to the client.
  void sendResponse(ResponseMessage response);
}

/// Instances of the class [LspByteStreamServerChannel] implement an
/// [LspServerCommunicationChannel] that uses a stream and a sink (typically,
/// standard input and standard output) to communicate with clients.
class LspByteStreamServerChannel implements LspServerCommunicationChannel {
  LspByteStreamServerChannel(this._input, this._output);

  final Stream<List<int>> _input;
  final IOSink _output;

  /// Completer that will be signalled when the input stream is closed.
  final _closed = Completer<void>();

  /// True if [close] has been called.
  bool _closeRequested = false;

  /// Future that will be completed when the input stream is closed.
  @override
  Future<void> get closed => _closed.future;

  @override
  void close() {
    if (!_closeRequested) {
      _closeRequested = true;
      assert(!_closed.isCompleted);
      _closed.complete();
    }
  }

  @override
  void listen(
    void Function(Message message) onMessage, {
    Function onError,
    void Function() onDone,
  }) {
    _input.transform(LspPacketTransformer()).listen(
      (data) => _readMessage(data, onMessage),
      onError: onError,
      onDone: () {
        close();
        onDone?.call();
      },
    );
  }

  @override
  void sendNotification(NotificationMessage notification) =>
      _sendLsp(notification.toJson());

  @override
  void sendRequest(RequestMessage request) => _sendLsp(request.toJson());

  @override
  void sendResponse(ResponseMessage response) => _sendLsp(response.toJson());

  /// Read a request from the given [data] and use the given function to handle
  /// the message.
  void _readMessage(String data, void Function(Message request) onMessage) {
    // Ignore any further requests after the communication channel is closed.
    if (_closed.isCompleted) {
      return;
    }
    final json = jsonDecode(data) as Map<String, dynamic>;
    if (RequestMessage.canParse(json, nullLspJsonReporter)) {
      onMessage(RequestMessage.fromJson(json));
    } else if (NotificationMessage.canParse(json, nullLspJsonReporter)) {
      onMessage(NotificationMessage.fromJson(json));
    } else if (ResponseMessage.canParse(json, nullLspJsonReporter)) {
      onMessage(ResponseMessage.fromJson(json));
    } else {
      _sendParseError();
    }
  }

  /// Sends a message prefixed with the required LSP headers.
  void _sendLsp(Map<String, dynamic> json) {
    // Don't send any further responses after the communication channel is
    // closed.
    if (_closeRequested) {
      return;
    }

    final jsonEncodedBody = jsonEncode(json);
    final utf8EncodedBody = utf8.encode(jsonEncodedBody);
    final header = 'Content-Length: ${utf8EncodedBody.length}\r\n'
        'Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n';
    final asciiEncodedHeader = ascii.encode(header);

    // Header is always ascii, body is always utf8!
    _write(asciiEncodedHeader);
    _write(utf8EncodedBody);
  }

  void _sendParseError() {
    final error = ResponseMessage(
      null,
      null,
      ResponseError<dynamic>(
        ErrorCodes.ParseError,
        'Unable to parse message',
        null,
      ),
      jsonRpcVersion,
    );
    sendResponse(error);
  }

  /// Send [bytes] to [_output].
  void _write(List<int> bytes) =>
      runZonedGuarded(() => _output.add(bytes), (e, s) => close());
}
