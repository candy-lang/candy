import { CancellationToken, CancellationTokenSource } from "vscode";

export type PromiseOr<T> = T | Promise<T>;

export function combineCancellationTokens(
  ...tokens: CancellationToken[]
): CancellationToken {
  const source = new CancellationTokenSource();
  tokens.forEach((token) =>
    token.onCancellationRequested(() => {
      source.cancel();
    }),
  );
  return source.token;
}
