import {
  DocumentUri,
  NotificationType,
  Position,
  RequestType,
} from 'vscode-languageclient';

// Debug IRs
export interface ViewIrParams {
  readonly uri: DocumentUri;
}
export const viewRcst = new RequestType<ViewIrParams, string, void>(
  'candy/viewRcst'
);
export const viewAst = new RequestType<ViewIrParams, string, void>(
  'candy/viewAst'
);
export const viewHir = new RequestType<ViewIrParams, string, void>(
  'candy/viewHir'
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
