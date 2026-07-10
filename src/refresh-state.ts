export type RefreshCallbacks<T> = {
  loading: (value: boolean) => void;
  success: (value: T) => void;
  failure: () => void;
};

async function runRefresh<T>(request: () => Promise<T>, callbacks: RefreshCallbacks<T>): Promise<void> {
  callbacks.loading(true);
  try {
    callbacks.success(await request());
  } catch {
    callbacks.failure();
  } finally {
    callbacks.loading(false);
  }
}

export function runUsageRefresh<T>(request: () => Promise<T>, callbacks: RefreshCallbacks<T>): Promise<void> {
  return runRefresh(request, callbacks);
}

export function runSpendRefresh<T>(request: () => Promise<T>, callbacks: RefreshCallbacks<T>): Promise<void> {
  return runRefresh(request, callbacks);
}
