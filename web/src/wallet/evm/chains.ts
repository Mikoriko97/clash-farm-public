// EVM chains enabled for login (ADR-012). Add a chain = add one entry here.
// Robinhood chain: add once public chain id / RPC are published.
import { arbitrum, base, mainnet, type Chain } from "viem/chains";

export const EVM_CHAINS: readonly Chain[] = [mainnet, base, arbitrum];

export function isSupportedChain(chainId: number): boolean {
  return EVM_CHAINS.some((chain) => chain.id === chainId);
}
