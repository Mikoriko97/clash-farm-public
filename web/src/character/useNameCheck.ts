// Live answer while the player types a nickname (ADR-025): the shape is judged locally, only a
// well-formed name is asked of the server (debounced, and capped per day over there).
import { useEffect, useState } from "react";
import { ApiError, getCharacterApi } from "../api";
import { humanError } from "../api/humanError";
import { nicknameIssue } from "./nickname";

export type NameStatus =
  | { kind: "idle" }
  | { kind: "checking"; nick: string }
  | { kind: "ok"; nick: string }
  | { kind: "bad"; nick: string; message: string }
  /** The server says this account already has its character (created elsewhere): reload it. */
  | { kind: "exists"; nick: string };

export const CHECK_DEBOUNCE_MS = 450;

export const NAME_ERRORS: Record<string, string> = {
  invalid_nickname: "That name doesn't fit the rules.",
  nickname_not_allowed: "That name is not allowed.",
  nickname_taken: "That name is taken. Pick another.",
  daily_cap: "Too many tries today. Come back tomorrow.",
  invalid_look: "That look is not available.",
};

/** The server words its reasons in lower case; a sentence starts with a capital. */
export function sentence(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export function useNameCheck(nick: string): NameStatus {
  // Only the server's answer is state; "idle", the shape verdict and "checking" follow from the name.
  const [remote, setRemote] = useState<NameStatus | null>(null);
  const issue = nick === "" ? null : nicknameIssue(nick);
  useEffect(() => {
    if (nick === "" || issue) return;
    let alive = true;
    const timer = window.setTimeout(() => {
      getCharacterApi()
        .check(nick)
        .then((r) => alive && setRemote(r.available ? { kind: "ok", nick } : { kind: "bad", nick, message: sentence(r.message) }))
        .catch((e: unknown) => {
          if (!alive) return;
          if (e instanceof ApiError && e.code === "character_exists") setRemote({ kind: "exists", nick });
          else setRemote({ kind: "bad", nick, message: humanError(e, NAME_ERRORS) });
        });
    }, CHECK_DEBOUNCE_MS);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [nick, issue]);
  if (nick === "") return { kind: "idle" };
  if (issue) return { kind: "bad", nick, message: issue };
  return remote && remote.kind !== "idle" && remote.nick === nick ? remote : { kind: "checking", nick };
}
