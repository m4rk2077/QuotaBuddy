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

export function createSingleFlightRefresh<T>(request: () => Promise<T>, callbacks: RefreshCallbacks<T>): () => Promise<void> {
  let inFlight: Promise<void> | null = null;
  return () => {
    if (inFlight) return inFlight;
    const current = runRefresh(request, callbacks).finally(() => {
      if (inFlight === current) inFlight = null;
    });
    inFlight = current;
    return current;
  };
}
