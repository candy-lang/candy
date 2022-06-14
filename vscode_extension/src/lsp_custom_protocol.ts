import { NotificationType, Position } from 'vscode-languageclient';

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
export type HintKind = 'value' | 'panic';
