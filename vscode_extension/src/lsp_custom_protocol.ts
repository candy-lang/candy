import * as vscode from 'vscode';
import {
  DocumentUri,
  NotificationType,
  Position,
  RequestType,
} from 'vscode-languageclient';

// Debug Adapter Protocol
export type DebugAdapterId = number;
export const debugAdapterCreate = new RequestType<{}, DebugAdapterId, void>(
  'candy/debugAdapter.create'
);
// VS Code ←→ Candy
export const debugAdapterMessage = new NotificationType<DebugAdapterMessage>(
  'candy/debugAdapter.message'
);
export interface DebugAdapterMessage {
  readonly debugAdapterId: DebugAdapterId;
  readonly message: vscode.DebugProtocolMessage;
}

// Debug IRs
export interface ViewIrParams {
  readonly uri: DocumentUri;
}
export const viewIr = new RequestType<ViewIrParams, string, void>(
  'candy/viewIr'
);

export const updateIrNotification = new NotificationType<UpdateIrParams>(
  'candy/updateIr'
);
export interface UpdateIrParams {
  readonly uri: DocumentUri;
}

// Hints
export class PublishHintsNotification {
  public static type = new NotificationType<HintsParams>(
    'candy/textDocument/publishHints'
  );
}
export interface HintsParams {
  readonly uri: string;
  readonly hints: Hint[];
}
export interface Hint {
  readonly kind: HintKind;
  readonly text: string;
  readonly position: Position;
}
export type HintKind = 'value' | 'panic' | 'fuzz';
