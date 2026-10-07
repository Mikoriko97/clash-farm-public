import { describe, expect, it } from "vitest";
import { parseFrame, presenceUrl, toGodotPeers } from "../usePresence";

describe("presence frames", () => {
  it("builds the socket url from the page origin", () => {
    expect(presenceUrl({ protocol: "https:", host: "play.example" })).toBe("wss://play.example/api/v1/ws/presence");
    expect(presenceUrl({ protocol: "http:", host: "localhost:5173" })).toBe("ws://localhost:5173/api/v1/ws/presence");
  });

  it("accepts hello and peers, drops malformed peers and garbage", () => {
    expect(parseFrame('{"t":"hello","id":"abc","nick":"Farmer #abcd"}')).toEqual({ t: "hello", id: "abc", nick: "Farmer #abcd", look: null });
    const peers = parseFrame('{"t":"peers","peers":[{"id":"a","nick":"n","x_dm":10,"z_dm":-5,"yaw_deg":90,"anim":"idle"},{"id":"b"},"junk"]}');
    expect(peers).toEqual({ t: "peers", peers: [{ id: "a", nick: "n", x_dm: 10, z_dm: -5, yaw_deg: 90, anim: "idle" }] });
    expect(parseFrame('{"t":"reset","zone":"mainland","x_dm":0,"z_dm":4000}')).toEqual({ t: "reset", zone: "mainland", x_dm: 0, z_dm: 4000 });
    expect(parseFrame('{"t":"reset","zone":"mainland"}')).toBeNull();
    // A moderator's reset (0031): the server says the name changed, the client reloads it.
    expect(parseFrame('{"t":"identity"}')).toEqual({ t: "identity" });
    expect(parseFrame("not json")).toBeNull();
    expect(parseFrame('{"t":"other"}')).toBeNull();
    expect(parseFrame("null")).toBeNull();
  });

  it("carries the look: kept when valid, dropped (the peer stays) when malformed, sent to Godot as colours", () => {
    const look = { body: 1, build: 2, skin: 1, shirt: 2, pants: 3, shoes: 4, hair_color: 5, hair: 2, face: 1, beard: 3, hat: 4, glasses: 1, outfit: 3, outfit_color: 6 };
    expect(parseFrame(JSON.stringify({ t: "hello", id: "abc", nick: "Mira", look }))).toEqual({ t: "hello", id: "abc", nick: "Mira", look });
    expect(parseFrame('{"t":"hello","id":"abc","nick":"Mira","look":{"body":2,"skin":0,"shirt":0,"pants":0,"shoes":0,"hair_color":0,"hair":0,"face":0,"beard":0,"hat":0,"glasses":0,"outfit":0,"outfit_color":0}}')).toMatchObject({ look: null });
    expect(parseFrame('{"t":"hello","id":"abc","nick":"Mira","look":{"body":0,"skin":99,"shirt":0,"pants":0,"shoes":0,"hair_color":0,"hair":0,"face":0,"beard":0,"hat":0,"glasses":0,"outfit":0,"outfit_color":0}}')).toMatchObject({ look: null });
    const pose = { x_dm: 10, z_dm: -5, yaw_deg: 90, anim: "idle" };
    const frame = parseFrame(
      JSON.stringify({ t: "peers", peers: [{ id: "a", nick: "A", look, ...pose }, { id: "b", nick: "B", look: { skin: -1 }, ...pose }, { id: "c", nick: "C", look: null, ...pose }] }),
    );
    expect(frame).toMatchObject({ t: "peers" });
    const peers = frame && frame.t === "peers" ? frame.peers : [];
    expect(peers.map((p) => [p.id, p.look ?? null])).toEqual([["a", look], ["b", null], ["c", null]].map(([id, l]) => [id, l]));
    const sent = toGodotPeers(peers);
    expect(sent[0]).toMatchObject({ id: "a", look: { body: "body_f", build: "build_sturdy", skin: "#f0c8a0", shirt: "#dd8a34", pants: "#566333", shoes: "#a33a32", hair_color: "#c8642a", hair: "hair_short", face: "face_smile", beard: "beard_full", hat: "hat_cowboy", glasses: "glasses_round", outfit: "outfit_hoodie", outfit_color: "#7a4fa3" } });
    expect(sent[1]).not.toHaveProperty("look");
  });
});
