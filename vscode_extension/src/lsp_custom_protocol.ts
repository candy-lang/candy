import {
  DocumentUri,
  NotificationType,
  Range,
  RequestType,
} from 'vscode-languageclient';

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
  readonly range: Range;
}
export type HintKind = 'value' | 'panic' | 'fuzz' | 'fuzzCallSite';
