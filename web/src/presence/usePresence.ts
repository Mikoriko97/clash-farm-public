// Presence socket (docs/MAINLAND.md §8, docs/API.md §Presence): Godot reports the
// character's pose, we forward it over one WebSocket with the cookie session,
// the server streams the neighbours back and Godot draws them. Reconnects with
// backoff while a session exists; nothing here touches the economy.
import { useEffect, useRef, useState } from "react";
import { getAuthApi } from "../api";
import { onFromGodot, sendToGodot } from "../bridge/host";
import { useAppStore } from "../store/appStore";
import { isLook, lookColors, type Look } from "../character/palette";
import { reloadCharacter } from "../character/useCharacter";
import { mockFlags } from "../mock/mode";

export const PRESENCE_PATH = "/api/v1/ws/presence";
/** Poses forwarded at most this often (the server refuses faster than 12/s). */
export const SEND_MIN_INTERVAL_MS = 120;
const BACKOFF_MS = [1000, 2000, 5000, 10000];

export interface PeerView {
  id: string;
  nick: string;
  x_dm: number;
  z_dm: number;
  yaw_deg: number;
  anim: string;
  /** Palette indexes; absent for accounts without a character. */
  look?: Look;
}

export interface PresenceState {
  connected: boolean;
  peers: number;
  me: { id: string; nick: string } | null;
}

/** ws(s)://<host>/api/v1/ws/presence for the page's own origin. */
export function presenceUrl(loc: { protocol: string; host: string } = window.location): string {
  const scheme = loc.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${loc.host}${PRESENCE_PATH}`;
}

/** Parses a server frame; null for anything that is not a well-formed peers/hello frame. */
export type PresenceFrame =
  | { t: "hello"; id: string; nick: string; look: Look | null }
  | { t: "peers"; peers: PeerView[] }
  | { t: "reset"; zone: string; x_dm: number; z_dm: number }
  | { t: "identity" };

export function parseFrame(text: string): PresenceFrame | null {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return null;
  }
  if (!raw || typeof raw !== "object") return null;
  const obj = raw as Record<string, unknown>;
  if (obj["t"] === "hello" && typeof obj["id"] === "string" && typeof obj["nick"] === "string") {
    return { t: "hello", id: obj["id"], nick: obj["nick"], look: isLook(obj["look"]) ? obj["look"] : null };
  }
  if (obj["t"] === "peers" && Array.isArray(obj["peers"])) {
    const peers = obj["peers"].filter(isPeer).map(withValidLook);
    return { t: "peers", peers };
  }
  // The name changed elsewhere (a moderator's reset, 0031): reload the character; the socket closes.
  if (obj["t"] === "identity") return { t: "identity" };
  if (obj["t"] === "reset" && typeof obj["zone"] === "string" && typeof obj["x_dm"] === "number" && typeof obj["z_dm"] === "number") {
    return { t: "reset", zone: obj["zone"], x_dm: obj["x_dm"], z_dm: obj["z_dm"] };
  }
  return null;
}

function isPeer(p: unknown): p is PeerView {
  if (!p || typeof p !== "object") return false;
  const o = p as Record<string, unknown>;
  return typeof o["id"] === "string" && typeof o["nick"] === "string" && typeof o["x_dm"] === "number" && typeof o["z_dm"] === "number" && typeof o["yaw_deg"] === "number" && typeof o["anim"] === "string";
}

/** A malformed `look` never reaches Godot: the peer is kept, without it. */
function withValidLook(p: PeerView): PeerView {
  if (p.look === undefined || isLook(p.look)) return p;
  const clean: PeerView = { ...p };
  delete clean.look;
  return clean;
}

/** The peers as Godot wants them: the look as colours, not indexes (`peers_state`, docs/BRIDGE_PROTOCOL.md). */
export function toGodotPeers(peers: PeerView[]): Record<string, unknown>[] {
  return peers.map(({ look, ...rest }) => (look ? { ...rest, look: lookColors(look) } : rest));
}

export function usePresence(): PresenceState {
  const session = useAppStore((s) => s.session);
  // A new character name reconnects the socket: the server reads the identity once, at the handshake.
  const nickname = useAppStore((s) => s.character?.nickname ?? null);
  // …and a new look (paid restyle, ADR-038): the server sends it to the others with `hello`/`peers`, read at the handshake too.
  const lookKey = useAppStore((s) => (s.character ? JSON.stringify(s.character.look) : null));
  const godotStatus = useAppStore((s) => s.godotStatus);
  const [state, setState] = useState<PresenceState>({ connected: false, peers: 0, me: null });
  const socket = useRef<WebSocket | null>(null);
  const lastSent = useRef(0);

  useEffect(() => {
    // Мок-режим: сесії на сервері нема, сокет не відкриваємо (інакше безкінечні відмови рукостискання).
    if (!session || godotStatus !== "ready" || mockFlags()) return;
    let alive = true;
    let attempt = 0;
    let timer: number | null = null;

    const open = () => {
      if (!alive) return;
      const ws = new WebSocket(presenceUrl());
      socket.current = ws;
      ws.onopen = () => {
        attempt = 0;
        setState((s) => ({ ...s, connected: true }));
      };
      ws.onmessage = (ev) => {
        const frame = parseFrame(String(ev.data));
        if (!frame) return;
        if (frame.t === "hello") setState((s) => ({ ...s, me: { id: frame.id, nick: frame.nick } }));
        else if (frame.t === "identity") void reloadCharacter();
        else if (frame.t === "reset") {
          // Same zone: just move the character. Another zone: the server refused the door — go back.
          if (frame.zone === useAppStore.getState().zone) sendToGodot("teleport", { zone: frame.zone, x_dm: frame.x_dm, z_dm: frame.z_dm });
          else sendToGodot("set_zone", { zone: frame.zone, x_dm: frame.x_dm, z_dm: frame.z_dm });
        }
        else {
          setState((s) => (s.peers === frame.peers.length ? s : { ...s, peers: frame.peers.length }));
          sendToGodot("peers_state", { peers: toGodotPeers(frame.peers) });
        }
      };
      ws.onclose = () => {
        // A socket closed by cleanup (StrictMode re-runs the effect) must not
        // clobber the newer one that replaced it.
        if (socket.current === ws) socket.current = null;
        if (!alive) return;
        setState((s) => ({ ...s, connected: false, peers: 0 }));
        sendToGodot("peers_state", { peers: [] });
        const wait = BACKOFF_MS[Math.min(attempt, BACKOFF_MS.length - 1)];
        attempt += 1;
        // The socket dies when the access token expires; an HTTP call refreshes it first.
        timer = window.setTimeout(() => void getAuthApi().me().catch(() => undefined).then(open), wait);
      };
      ws.onerror = () => ws.close();
    };
    open();

    const stop = onFromGodot((m) => {
      if (m.type !== "player_pose") return;
      const ws = socket.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) return;
      const now = Date.now();
      if (now - lastSent.current < SEND_MIN_INTERVAL_MS) return;
      lastSent.current = now;
      const p = m.payload;
      ws.send(JSON.stringify({ t: "pos", zone: useAppStore.getState().zone, x_dm: Number(p["x_dm"]), z_dm: Number(p["z_dm"]), yaw_deg: Number(p["yaw_deg"]), anim: String(p["anim"]) }));
    });

    return () => {
      alive = false;
      stop();
      if (timer !== null) window.clearTimeout(timer);
      socket.current?.close();
      socket.current = null;
    };
  }, [session, godotStatus, nickname, lookKey]);

  return state;
}
