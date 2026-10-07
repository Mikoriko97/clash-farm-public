/** Loads something once on first use and shares it: every caller awaits the same promise,
 *  so a second click while loading cannot start a second copy. A failed load is not
 *  cached — the next call tries again (a chunk that failed on a flaky network). Used for
 *  the wallet and Solana chunks that stay out of the first bundle. */
export function lazyOnce<T>(load: () => Promise<T>): () => Promise<T> {
  let pending: Promise<T> | null = null;
  return () => {
    pending ??= load().catch((error: unknown) => {
      pending = null;
      throw error;
    });
    return pending;
  };
}
