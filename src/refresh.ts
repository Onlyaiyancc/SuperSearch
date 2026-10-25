// 相同资源最多保留一次在途读取和一次后续刷新，调用者共同等待最新结果。
export function createLatestRefresh<T>(read: () => Promise<T>, changed: (value: T) => void, failed: (value: unknown) => void) {
  let inFlight: Promise<void> | null = null;
  let pending = false;
  let disposed = false;
  return {
    run(): Promise<void> {
      if (disposed) return Promise.resolve();
      if (inFlight) { pending = true; return inFlight; }
      let resolve!: () => void;
      let reject!: (value: unknown) => void;
      const completion = new Promise<void>((accept, refuse) => { resolve = accept; reject = refuse; });
      inFlight = completion;
      void (async () => {
        try {
          do {
            pending = false;
            try {
              const value = await read();
              if (!disposed && !pending) changed(value);
            } catch (value) { if (!disposed && !pending) failed(value); }
          } while (!disposed && pending);
        } finally { inFlight = null; }
      })().then(resolve, reject);
      return completion;
    },
    dispose() { disposed = true; pending = false; },
  };
}
