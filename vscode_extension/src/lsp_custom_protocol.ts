import * as vscode from 'vscode';
import {
  DocumentUri,
  NotificationType,
  Position,
  Range,
  RequestType,
} from 'vscode-languageclient';

// Debug Adapter Protocol
export type DebugSessionId = string;

export const debugAdapterCreate = new RequestType<
  DebugAdapterCreateParams,
  void,
  void
>('candy/debugAdapter/create');
export interface DebugAdapterCreateParams {
  readonly sessionId: DebugSessionId;
}

// VS Code ←→ Candy
export const debugAdapterMessage = new NotificationType<DebugAdapterMessage>(
  'candy/debugAdapter/message'
);
export interface DebugAdapterMessage {
  readonly sessionId: DebugSessionId;
  readonly message: vscode.DebugProtocolMessage;
}

// Debug IRs
export interface ViewIrParams {
  readonly uri: DocumentUri;
}
export const viewIr = new RequestType<ViewIrParams, string, void>(
  'candy/viewIr'
);

export const updateIrType = new NotificationType<UpdateIrParams>(
  'candy/updateIr'
);
export interface UpdateIrParams {
  readonly uri: DocumentUri;
}

// Hints
export const publishHintsType = new NotificationType<HintsParams>(
  'candy/textDocument/publishHints'
);
export interface HintsParams {
  readonly uri: string;
  readonly hints: Hint[];
}
export interface Hint {
  readonly kind: HintKind;
  readonly text: string;
  readonly position: Position;
}
export type HintKind =
  | 'value'
  | 'fuzzingStatus'
  | 'sampleInputReturningNormally'
  | 'sampleInputPanickingWithCallerResponsible'
  | 'sampleInputPanickingWithInternalCodeResponsible';

// Coverage
export const publishCoverageType = new NotificationType<CoverageParams>(
  'candy/textDocument/publishCoverage'
);
export interface CoverageParams {
  readonly uri: string;
  readonly ranges: Range[];
}

// Status
export const publishServerStatusType = new NotificationType<ServerStatus>(
  'candy/publishServerStatus'
);
export interface ServerStatus {
  text: string;
}
