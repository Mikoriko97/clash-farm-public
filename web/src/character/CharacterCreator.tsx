// First-login screen (ADR-025): pick a name and a look while the real character stands in the
// world behind the panel (Godot: camera_preview + player_look). The name is permanent, so the
// button stays off until the server has said the name is free.
import { useEffect, useState } from "react";
import { ApiError, getAuthApi, getCharacterApi } from "../api";
import { humanError } from "../api/humanError";
import { useAppStore } from "../store/appStore";
import { NICK_MAX } from "./nickname";
import type { TabId } from "./CreatorPickers";
import { LookPicker } from "./LookPicker";
import { randomLook, type Look } from "./palette";
import { NAME_ERRORS, type NameStatus, useNameCheck } from "./useNameCheck";
import { useLookPreview } from "./useLookPreview";
import { characterOf } from "./useCharacter";
import "./creator.css";

export function statusLine(status: NameStatus): { text: string; tone: "ok" | "bad" | "wait" | "" } {
  if (status.kind === "checking") return { text: "Checking…", tone: "wait" };
  if (status.kind === "ok") return { text: `${status.nick} is free`, tone: "ok" };
  if (status.kind === "bad") return { text: status.message, tone: "bad" };
  if (status.kind === "exists") return { text: "This account already has a character.", tone: "bad" };
  return { text: "3–16 letters, digits or _", tone: "" };
}

export function CharacterCreator() {
  const setCharacter = useAppStore((s) => s.setCharacter);
  const [nick, setNick] = useState("");
  const [look, setLook] = useState<Look>(() => randomLook());
  const [yaw, setYaw] = useState(0);
  const [tab, setTab] = useState<TabId>("body");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const status = useNameCheck(nick);
  const line = statusLine(status);
  const canCreate = status.kind === "ok" && status.nick === nick && !busy;

  // The camera comes close for as long as the panel is open, and turns with the arrows.
  useLookPreview(look, yaw, tab);

  // The account already has its character (made in another tab): take that one.
  useEffect(() => {
    if (status.kind === "exists") void adopt(setCharacter, setError);
  }, [status.kind, setCharacter]);

  const submit = async () => {
    if (!canCreate) return;
    setBusy(true);
    setError(null);
    try {
      const made = await getCharacterApi().create(nick, look);
      setCharacter({ nickname: made.nickname, look: made.look });
    } catch (e) {
      if (e instanceof ApiError && e.code === "character_exists") void adopt(setCharacter, setError);
      else setError(humanError(e, NAME_ERRORS));
      setBusy(false);
    }
  };

  return (
    <form
      className="creator glass"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <h2>Create your character</h2>
      <p className="cc-hint">Everyone you meet sees this name above your head. You can&apos;t change it later.</p>
      <label className="cc-field">
        <span>Name</span>
        <input
          value={nick}
          maxLength={NICK_MAX}
          autoFocus
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
          placeholder="Your name"
          aria-describedby="name-status"
          aria-invalid={line.tone === "bad"}
          onChange={(e) => setNick(e.target.value.trim())}
        />
        <small id="name-status" className={`cc-status ${line.tone}`} role="status" aria-live="polite">
          {line.text}
        </small>
      </label>
      <LookPicker look={look} onChange={setLook} tab={tab} onTab={setTab} onTurn={(d) => setYaw((y) => y + d)} />
      <div className="cc-foot">
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <button type="submit" className="btn primary" disabled={!canCreate}>
          {busy ? "Creating…" : "Start playing"}
        </button>
      </div>
    </form>
  );
}

async function adopt(setCharacter: ReturnType<typeof useAppStore.getState>["setCharacter"], fail: (message: string) => void) {
  try {
    const { user } = await getAuthApi().me();
    setCharacter(characterOf(user));
  } catch {
    fail("Could not load your character. Reload the page.");
  }
}
