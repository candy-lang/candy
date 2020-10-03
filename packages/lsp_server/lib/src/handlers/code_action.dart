import 'dart:async';

import '../analysis_server.dart';
import '../generated/lsp_protocol/protocol_generated.dart';
import '../generated/lsp_protocol/protocol_special.dart';
import 'handlers.dart';

abstract class CandyCodeActionKind {
  static const serverSupportedKinds = [build, run];
  static const build = CodeActionKind('build');
  static const run = CodeActionKind('run');
}

class CodeActionHandler extends MessageHandler<CodeActionParams,
    List<Either2<Command, CodeAction>>> {
  CodeActionHandler(AnalysisServer server) : super(server);
  @override
  Method get handlesMessage => Method.textDocument_codeAction;

  @override
  LspJsonHandler<CodeActionParams> get jsonHandler =>
      CodeActionParams.jsonHandler;

  @override
  Future<ErrorOr<List<Either2<Command, CodeAction>>>> handle(
    CodeActionParams params,
    CancellationToken token,
  ) async {
    return success([
      Either2.t1(Command('Build', CandyCodeActionKind.build.toString(), null)),
      Either2.t1(Command('Run', CandyCodeActionKind.run.toString(), null)),
    ]);
  }
}
