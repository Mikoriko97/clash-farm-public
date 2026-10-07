// Character creation (ADR-025, docs/API.md): the nickname and look, chosen once; a new look later
// costs a restyle ticket (0029, ADR-038).
import type { ApiClient } from "./client";
import type { Look } from "../character/palette";

export interface Character {
  nickname: string;
  look: Look;
  /** A moderator reset the name (0031, ADR-040): the next rename needs no ticket. From `/me` only. */
  renameFree?: boolean;
}

export type NickCheck = { available: true } | { available: false; reason: "invalid" | "not_allowed" | "taken"; message: string };

export interface CharacterApi {
  create(nickname: string, look: Look): Promise<Character>;
  check(nickname: string): Promise<NickCheck>;
  /** `POST /me/look`: spends one restyle ticket; the name stays. */
  restyle(look: Look): Promise<Character>;
  /** `POST /me/nickname`: spends one rename ticket (0030, ADR-039). */
  rename(nickname: string): Promise<Character>;
}

export function createCharacterApi(client: ApiClient): CharacterApi {
  return {
    create: (nickname, look) => client.post("/me/character", { nickname, look }),
    check: (nickname) => client.get(`/me/nickname-check?nick=${encodeURIComponent(nickname)}`),
    restyle: (look) => client.post("/me/look", { look }),
    rename: (nickname) => client.post("/me/nickname", { nickname }),
  };
}
