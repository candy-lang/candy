import { NotificationType, Range } from "vscode-languageclient";

export class PublishTypeLabelsNotification {
  public static type = new NotificationType<TypeLabelsParams>(
    "candy/textDocument/publishTypeLabels"
  );
}
export interface TypeLabelsParams {
  readonly uri: string;
  readonly labels: TypeLabel[];
}
export interface TypeLabel {
  readonly label: string;
  readonly range: Range;
}
