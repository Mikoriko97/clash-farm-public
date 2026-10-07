// Read-only CLASH balance of the connected Solana wallet (docs/UI.md §5.2):
// shown only when > 0. One RPC call per minute and after each purchase.
import { useEffect, useState } from "react";
import { FAKE_CLASH_BALANCE, fakeAllowed } from "../payments/fake";
import { loadSolana } from "../payments/loadSolana";
import { useAppStore } from "../store/appStore";

export const CLASH_MINT = "9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump";
export const CLASH_DECIMALS = 6;
const REFRESH_MS = 60_000;
/** Публічний RPC Solana віддає браузерам 403 (і бачить адресу гаманця): без власного
 *  `VITE_SOLANA_RPC_URL` баланс не опитуємо — чіп просто прихований. */
const RPC_CONFIGURED = Boolean((import.meta.env as Record<string, string | undefined>)["VITE_SOLANA_RPC_URL"]);

export function useClashBalance(refreshKey: unknown): bigint {
  const wallet = useAppStore((s) => s.wallet);
  const session = useAppStore((s) => s.session);
  const [balance, setBalance] = useState<bigint>(0n);
  const address = wallet && wallet.kind === "sol" && session ? wallet.address : null;
  const fake = fakeAllowed(useAppStore((s) => s.paymentsFake));

  useEffect(() => {
    if (!address || fake || !RPC_CONFIGURED) return;
    let alive = true;
    let failures = 0;
    const load = async () => {
      // Три збої поспіль — RPC недоступний: не засмічуємо консоль щохвилини (оновиться після покупки).
      if (failures >= 3) return;
      try {
        const { getAssociatedTokenAddressSync, PublicKey, getConnection } = await loadSolana();
        const ata = getAssociatedTokenAddressSync(new PublicKey(CLASH_MINT), new PublicKey(address));
        const { value } = await getConnection().getTokenAccountBalance(ata, "confirmed");
        failures = 0;
        if (alive) setBalance(BigInt(value.amount));
      } catch {
        failures += 1;
        if (alive) setBalance(0n); // no token account or RPC down: simply hide the chip
      }
    };
    void load();
    const id = window.setInterval(() => void load(), REFRESH_MS);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, [address, refreshKey, fake]);

  if (fake && address) return FAKE_CLASH_BALANCE;
  return address ? balance : 0n;
}

/** "12 500" style grouping without floats; whole tokens only for the chip. */
export function formatWhole(atomic: bigint, decimals: number): string {
  const whole = atomic / 10n ** BigInt(decimals);
  return whole.toString().replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}
