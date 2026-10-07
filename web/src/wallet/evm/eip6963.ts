// EVM wallets via EIP-6963 provider discovery (MetaMask, Rabby, Coinbase, Phantom-EVM...).
// We never touch window.ethereum directly: with several extensions installed it is
// whichever injected last, which is exactly the spoofing surface EIP-6963 removes.
import { createWalletClient, custom, hexToBytes, type EIP1193Provider, type WalletClient } from "viem";
import { safeIcon, type WalletBackend, WalletError, type WalletOption } from "../types";
import { EVM_CHAINS, isSupportedChain } from "./chains";

export interface Eip6963ProviderInfo {
  uuid: string;
  name: string;
  icon: string;
  rdns: string;
}
export interface Eip6963ProviderDetail {
  info: Eip6963ProviderInfo;
  provider: EIP1193Provider;
}

const ANNOUNCE = "eip6963:announceProvider";
const REQUEST = "eip6963:requestProvider";

function isDetail(value: unknown): value is Eip6963ProviderDetail {
  if (typeof value !== "object" || value === null) return false;
  const detail = value as { info?: Partial<Eip6963ProviderInfo>; provider?: unknown };
  return (
    typeof detail.info?.uuid === "string" &&
    typeof detail.info.name === "string" &&
    typeof detail.info.rdns === "string" &&
    typeof detail.provider === "object" &&
    detail.provider !== null &&
    typeof (detail.provider as { request?: unknown }).request === "function"
  );
}

interface ActiveEvm {
  detail: Eip6963ProviderDetail;
  client: WalletClient;
  address: `0x${string}`;
}

/** `target` is `window` in the app and a bare EventTarget in tests. */
export function createEvmBackend(target: EventTarget): WalletBackend {
  const providers = new Map<string, Eip6963ProviderDetail>();
  const listeners = new Set<() => void>();
  let current: ActiveEvm | null = null;

  target.addEventListener(ANNOUNCE, (event) => {
    const detail = (event as CustomEvent<unknown>).detail;
    if (!isDetail(detail) || providers.has(detail.info.uuid)) return;
    providers.set(detail.info.uuid, detail);
    for (const listener of listeners) listener();
  });
  target.dispatchEvent(new Event(REQUEST));

  return {
    kind: "evm",
    list: () =>
      [...providers.values()].map<WalletOption>((p) => ({
        id: p.info.uuid,
        kind: "evm",
        name: p.info.name,
        icon: safeIcon(p.info.icon),
      })),
    onChange: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    async connect(id) {
      const detail = providers.get(id);
      if (!detail) throw new WalletError("not_found", `EVM wallet "${id}" not detected`);
      const client = createWalletClient({ transport: custom(detail.provider) });
      const [address] = await client.requestAddresses();
      if (!address) throw new WalletError("no_accounts", "wallet returned no accounts");
      const chainId = await client.getChainId();
      if (!isSupportedChain(chainId)) {
        const allowed = EVM_CHAINS.map((c) => `${c.name} (${c.id})`).join(", ");
        throw new WalletError("unsupported", `chain ${chainId} is not supported; switch to ${allowed}`);
      }
      current = { detail, client, address };
      return { id, kind: "evm", name: detail.info.name, address, chainId };
    },
    async disconnect() {
      // EIP-1193 has no standard disconnect; dropping our reference is the disconnect.
      current = null;
    },
    async signMessage(message) {
      if (!current) throw new WalletError("not_connected", "no EVM wallet connected");
      const hex = await current.client.signMessage({ account: current.address, message: { raw: message } });
      return hexToBytes(hex);
    },
  };
}
