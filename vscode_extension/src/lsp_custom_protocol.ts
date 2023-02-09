import {
  DocumentUri,
  NotificationType,
  Position,
  RequestType,
} from 'vscode-languageclient';

// Debug IRs
export interface ViewRcstParams {
  readonly uri: DocumentUri;
}
export const viewRcst = new RequestType<ViewRcstParams, string, void>(
  'candy/viewRcst'
);

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
