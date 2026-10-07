// The account's character (ADR-025): loaded from /me once per session, and kept in step with
// Godot (the local character wears its look; the presence socket carries it to the others).
import { useEffect } from "react";
import { getAuthApi } from "../api";
import type { MeResponse } from "../api/auth";
import type { Character } from "../api/character";
import { sendToGodot } from "../bridge/host";
import { useAppStore } from "../store/appStore";
import { isLook, lookColors } from "./palette";

const RETRY_MS = 2500;

/** The character from a `/me` answer; `null` until the account has created one. */
export function characterOf(user: MeResponse["user"]): Character | null {
  return user.nickname && isLook(user.look) ? { nickname: user.nickname, look: user.look, renameFree: user.rename_free === true } : null;
}

/** Re-reads the character: the presence socket says the name changed elsewhere (a moderator's reset, 0031). */
export function reloadCharacter(): Promise<void> {
  return getAuthApi()
    .me()
    .then(({ user }) => useAppStore.getState().setCharacter(characterOf(user)))
    .catch(() => undefined);
}

/** `undefined` while /me loads, `null` when the account has no character yet, else the character. */
export function useCharacter() {
  const session = useAppStore((s) => s.session);
  const character = useAppStore((s) => s.character);
  const setCharacter = useAppStore((s) => s.setCharacter);
  const godotStatus = useAppStore((s) => s.godotStatus);

  useEffect(() => {
    if (!session) return;
    let alive = true;
    let timer: number | null = null;
    const load = () => {
      getAuthApi()
        .me()
        .then(({ user }) => {
          if (alive) setCharacter(characterOf(user));
        })
        .catch(() => {
          if (alive) timer = window.setTimeout(load, RETRY_MS);
        });
    };
    load();
    return () => {
      alive = false;
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [session, setCharacter]);

  // Godot keeps the last look for scenes that appear later (zone changes).
  useEffect(() => {
    if (godotStatus === "ready" && character) sendToGodot("player_look", { look: lookColors(character.look) });
  }, [godotStatus, character]);

  return character;
}
