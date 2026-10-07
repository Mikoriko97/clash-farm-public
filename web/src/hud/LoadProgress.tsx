/** Смуга завантаження рушія: частка (0, поки розмір невідомий) і скільки байтів уже прийшло. Спільна для Welcome і завіси. */
export function LoadProgress({ progress, bytes }: { progress: number; bytes: number }) {
  const pct = Math.round(progress * 100);
  return (
    <div className="load" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress > 0 ? pct : undefined}>
      <span className={`load-bar${progress > 0 ? "" : " indeterminate"}`} style={progress > 0 ? { width: `${pct}%` } : undefined} />
      <span className="load-text">{progress > 0 ? `Loading the world ${pct}%` : `Loading the world · ${(bytes / 1_048_576).toFixed(1)} MB`}</span>
    </div>
  );
}
