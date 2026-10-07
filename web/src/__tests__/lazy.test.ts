import { describe, expect, it } from "vitest";
import { lazyOnce } from "../lazy";

describe("lazyOnce", () => {
  it("loads once and shares the result between parallel callers", async () => {
    let calls = 0;
    const load = lazyOnce(async () => {
      calls += 1;
      return { id: calls };
    });
    const [a, b] = await Promise.all([load(), load()]);
    expect(a).toBe(b);
    expect(await load()).toBe(a);
    expect(calls).toBe(1);
  });

  it("does not cache a failure: the next call loads again", async () => {
    let calls = 0;
    const load = lazyOnce(async () => {
      calls += 1;
      if (calls === 1) throw new Error("chunk failed");
      return "ok";
    });
    await expect(load()).rejects.toThrow("chunk failed");
    await expect(load()).resolves.toBe("ok");
    expect(calls).toBe(2);
  });

  it("callers waiting on a failed load all see the failure, then one retry succeeds for all", async () => {
    let calls = 0;
    const load = lazyOnce(async () => {
      calls += 1;
      if (calls === 1) throw new Error("offline");
      return calls;
    });
    const first = await Promise.allSettled([load(), load()]);
    expect(first.map((r) => r.status)).toEqual(["rejected", "rejected"]);
    const [a, b] = await Promise.all([load(), load()]);
    expect([a, b]).toEqual([2, 2]);
  });
});
