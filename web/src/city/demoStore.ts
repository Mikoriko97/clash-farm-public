/** Стан демо-гаманця: тестові баланси й позиції лише в памʼяті вкладки. Жодної мережі,
 *  жодних транзакцій — кнопки екранів міняють тільки цей стор. */
import { create } from "zustand";
import { LST_RATE, START_BALANCES, START_POSITIONS, SWAP, TOKENS, type TokenId, VAULTS } from "./demoData";
import { lstToSol, mulDiv, quoteSwap, solToLst } from "./demoMath";

export interface Position {
  deposited: number;
  earned: number;
}

export interface DemoState {
  balances: Record<TokenId, number>;
  positions: Record<string, Position>;
}

export type DemoResult = { ok: true; state: DemoState; got: number } | { ok: false; error: string };

const fail = (error: string): DemoResult => ({ ok: false, error });

function move(state: DemoState, from: TokenId, spend: number, to: TokenId, get: number): DemoState {
  const balances = { ...state.balances };
  balances[from] -= spend;
  balances[to] += get;
  return { ...state, balances };
}

export function applyStake(state: DemoState, sol: number): DemoResult {
  if (sol <= 0) return fail("Enter an amount");
  if (sol > state.balances.SOL) return fail("Not enough SOL");
  const got = solToLst(sol, LST_RATE);
  return { ok: true, state: move(state, "SOL", sol, "clashSOL", got), got };
}

export function applyUnstake(state: DemoState, lst: number, feeBps: number): DemoResult {
  if (lst <= 0) return fail("Enter an amount");
  if (lst > state.balances.clashSOL) return fail("Not enough clashSOL");
  const gross = lstToSol(lst, LST_RATE);
  const got = gross - mulDiv(gross, feeBps, 10_000);
  return { ok: true, state: move(state, "clashSOL", lst, "SOL", got), got };
}

export function applySwap(state: DemoState, from: TokenId, to: TokenId, amount: number, slippageBps: number): DemoResult {
  if (from === to) return fail("Pick two different tokens");
  if (amount <= 0) return fail("Enter an amount");
  if (amount > state.balances[from]) return fail(`Not enough ${from}`);
  const quote = quoteSwap(amount, TOKENS[from].price, TOKENS[to].price, SWAP.feeBps, slippageBps);
  if (quote.out <= 0) return fail("Amount too small");
  return { ok: true, state: move(state, from, amount, to, quote.out), got: quote.out };
}

export function applyDeposit(state: DemoState, vaultId: string, amount: number): DemoResult {
  const vault = VAULTS.find((v) => v.id === vaultId);
  if (!vault) return fail("No such pool");
  if (amount < vault.minDeposit) return fail("Below the minimum deposit");
  if (amount > state.balances[vault.token]) return fail(`Not enough ${vault.token}`);
  if (vault.cap > 0 && vault.tvl + amount > vault.cap) return fail("Pool is full");
  const prev = state.positions[vaultId] ?? { deposited: 0, earned: 0 };
  const balances = { ...state.balances, [vault.token]: state.balances[vault.token] - amount };
  return { ok: true, state: { balances, positions: { ...state.positions, [vaultId]: { ...prev, deposited: prev.deposited + amount } } }, got: amount };
}

/** Вивід частини внеску; разом з останньою частиною виходить і нарахований дохід. */
export function applyWithdraw(state: DemoState, vaultId: string, amount: number): DemoResult {
  const vault = VAULTS.find((v) => v.id === vaultId);
  const prev = state.positions[vaultId];
  if (!vault || !prev) return fail("No position");
  if (amount <= 0) return fail("Enter an amount");
  if (amount > prev.deposited) return fail("More than deposited");
  const closing = amount === prev.deposited;
  const got = amount + (closing ? prev.earned : 0);
  const positions = { ...state.positions };
  if (closing) delete positions[vaultId];
  else positions[vaultId] = { ...prev, deposited: prev.deposited - amount };
  return { ok: true, state: { balances: { ...state.balances, [vault.token]: state.balances[vault.token] + got }, positions }, got };
}

export function initialDemoState(): DemoState {
  return { balances: { ...START_BALANCES }, positions: structuredClone(START_POSITIONS) };
}

interface DemoStore extends DemoState {
  /** Застосовує чисту операцію; повертає її результат, щоб екран показав підсумок чи помилку. */
  run: (op: (state: DemoState) => DemoResult) => DemoResult;
  reset: () => void;
}

export const useDemoStore = create<DemoStore>((set, get) => ({
  ...initialDemoState(),
  run: (op) => {
    const { balances, positions } = get();
    const result = op({ balances, positions });
    if (result.ok) set(result.state);
    return result;
  },
  reset: () => set(initialDemoState()),
}));
