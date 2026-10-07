// One toast line for the whole HUD: island and mainland notices and errors feed it, the latest
// message wins and every toast clears itself — an old error can no longer hide later notices.
import { useEffect, useState } from "react";
import { sendToGodot } from "../bridge/host";
import { playUi } from "./sound";

export type ToastKind = "notice" | "error";

/** One feed: `key` changes with every new message (null = nothing to say). */
export interface ToastSource {
  key: string | null;
  text: string | null;
  kind: ToastKind;
}

export interface Toast {
  id: number;
  text: string;
  kind: ToastKind;
}

/** How long a toast stays: errors a little longer, they say what to do. */
export const TOAST_MS: Record<ToastKind, number> = { notice: 3_500, error: 6_000 };

/** The newest message among sources whose key changed since `seen`; with several at once the last
 *  source wins (callers list errors after notices). Null when nothing new arrived. */
export function latestToast(seen: (string | null)[], sources: ToastSource[]): ToastSource | null {
  let hit: ToastSource | null = null;
  sources.forEach((s, i) => {
    if (s.key !== null && s.text && s.key !== (seen[i] ?? null)) hit = s;
  });
  return hit;
}

export function notice(n: { key: string; text: string } | null): ToastSource {
  return { key: n?.key ?? null, text: n?.text ?? null, kind: "notice" };
}

export function error(text: string | null): ToastSource {
  return { key: text, text, kind: "error" };
}

/** The toast to show now; sounds (and the head shake on errors) play once per toast. */
export function useToast(sources: ToastSource[]): Toast | null {
  const keys = sources.map((s) => s.key);
  const [seen, setSeen] = useState<(string | null)[]>(() => sources.map(() => null));
  const [toast, setToast] = useState<Toast | null>(null);
  const [seq, setSeq] = useState(0);
  // A new message replaces the current toast (state from changed inputs, no effect).
  if (keys.length !== seen.length || keys.some((k, i) => k !== seen[i])) {
    setSeen(keys);
    const hit = latestToast(seen, sources);
    if (hit?.text) {
      setSeq(seq + 1);
      setToast({ id: seq + 1, text: hit.text, kind: hit.kind });
    }
  }
  useEffect(() => {
    if (!toast) return;
    if (toast.kind === "error") {
      playUi("error");
      sendToGodot("play_fx", { fx: "fail" }); // персонаж хитає головою (ADR-035)
    } else playUi("toast");
    const t = window.setTimeout(() => setToast(null), TOAST_MS[toast.kind]);
    return () => window.clearTimeout(t);
  }, [toast]);
  return toast;
}
