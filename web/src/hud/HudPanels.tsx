// Every sheet and drawer the HUD can open, one at a time (`panel`), plus the payment card.
import { RENAME_TICKET, RenameSheet } from "../character/RenameSheet";
import { RESTYLE_TICKET, RestyleSheet } from "../character/RestyleSheet";
import { CityDoor } from "../city/CityDoor";
import type { useIsland } from "../island/useIsland";
import type { MainlandState } from "../mainland/useMainland";
import type { usePurchase } from "../payments/usePurchase";
import { useAppStore } from "../store/appStore";
import type { Panel } from "./Chrome";
import { MarketSheet, WorkshopSheet } from "./CitySheets";
import { CraftSheet } from "./Craft";
import { HouseSheet } from "./House";
import { JournalSheet } from "./JournalSheet";
import { MapSheet } from "./MapSheet";
import { LinkWalletSheet } from "./GuestLink";
import { InventoryDrawer, MenuSheet } from "./panels";
import { PaySheet } from "./PaySheet";
import { ShopSheet } from "./Shop";
import { clashGate } from "./clashPayments";
import type { Building } from "./useBuildMode";

interface Props {
  panel: Panel;
  setPanel: (panel: Panel) => void;
  world: ReturnType<typeof useIsland>;
  mainland: MainlandState;
  purchase: ReturnType<typeof usePurchase>;
  seed: string | null;
  setSeed: (id: string | null) => void;
  setBuilding: (building: Building | null) => void;
  /** Null for a guest island: CLASH purchases open the link sheet instead of a payment. */
  wallet: string | null;
  version: string | null;
  onLogout: () => void;
}

export function HudPanels({ panel, setPanel, world, mainland, purchase, seed, setSeed, setBuilding, wallet, version, onLogout }: Props) {
  const tools = useAppStore((s) => s.tools);
  const mainlandMap = useAppStore((s) => s.mainlandMap);
  const explored = useAppStore((s) => s.explored);
  const playerChunk = useAppStore((s) => s.playerChunk);
  const landmarks = useAppStore((s) => s.landmarks);
  const totalLandmarks = useAppStore((s) => s.totalLandmarks);
  const expeditions = useAppStore((s) => s.expeditions);
  const cityBuilding = useAppStore((s) => s.building);
  const walletInfo = useAppStore((s) => s.wallet);
  const character = useAppStore((s) => s.character);
  const close = () => setPanel("none");
  const busy = world.busy !== null;
  const paymentsAvailable = useAppStore((s) => s.paymentsAvailable);
  const clash = clashGate(paymentsAvailable, world.catalog, wallet);
  const paymentsEnabled = clash.enabled;
  return (
    <>
      {panel === "inventory" && (
        <InventoryDrawer
          inventory={world.inventory}
          catalog={world.catalog}
          selectedSeed={seed}
          onSelectSeed={setSeed}
          busy={busy}
          onSell={(id, qty) => void world.sell(id, qty)}
          onEat={(id) => void world.eat(id)}
          onPlaceDecor={(id) => {
            const footprint = Number(world.catalog.find((c) => c.id === id)?.meta["footprint_dm"] ?? 10);
            setBuilding({ kind: "decor", item: id, footprint_dm: footprint });
            close();
          }}
          tools={tools}
          onRepair={(id) => void mainland.repair(id)}
          onClose={close}
        />
      )}
      {panel === "house" && <HouseSheet busy={busy} onSleep={() => void world.sleep().then((ok) => ok && close())} onCook={() => setPanel("craft")} onClose={close} />}
      {panel === "map" && <MapSheet map={mainlandMap} explored={explored} here={playerChunk} zone={mainland.zone === "cave" ? "island" : mainland.zone} landmarks={landmarks} onClose={close} />}
      {panel === "market" && <MarketSheet inventory={world.inventory} catalog={world.catalog} busy={busy} onSell={(id, qty) => void world.sell(id, qty)} onClose={close} />}
      {panel === "workshop" && (
        <WorkshopSheet recipes={world.recipes} inventory={world.inventory} tools={tools} busy={busy || mainland.busy} onCraft={(id) => void world.craft(id)} onRepair={(id) => void mainland.repair(id)} onClose={close} />
      )}
      {panel === "showcase" && <CityDoor building={cityBuilding} onClose={close} />}
      {panel === "journal" && (
        <JournalSheet expeditions={expeditions} landmarks={landmarks} totalLandmarks={totalLandmarks} busy={mainland.busy} onClaim={(i) => void mainland.claim(i)} onClose={close} />
      )}
      {panel === "craft" && (
        <CraftSheet recipes={world.recipes} inventory={world.inventory} catalog={world.catalog} level={world.island?.island.level ?? 1} busy={busy} onCraft={(id) => void world.craft(id)} onClose={close} />
      )}
      {panel === "shop" && purchase.step.kind === "idle" && (
        <ShopSheet
          catalog={world.catalog}
          credits={world.credits}
          busy={busy}
          paymentsEnabled={paymentsEnabled}
          paymentsReason={clash.reason}
          island={world.island?.island ?? null}
          inventory={world.inventory}
          plotPrice={world.island?.plot_price ?? 0}
          plotLimit={world.island?.plot_limit ?? null}
          placedPlots={world.island?.plots.length ?? 0}
          onBuyCredits={(id, qty) => void world.buy(id, qty)}
          onBuyClash={(id, qty) => (wallet === null ? setPanel("link") : void purchase.start(id, qty))}
          onExpand={(side) => void world.expand(side)}
          onClose={close}
        />
      )}
      {panel === "link" && <LinkWalletSheet onClose={close} />}
      {purchase.step.kind !== "idle" && <PaySheet step={purchase.step} onConfirm={() => void purchase.confirm()} onRecheck={() => void purchase.recheck()} onClose={purchase.reset} />}
      {panel === "menu" && (
        <MenuSheet
          wallet={wallet}
          walletName={walletInfo?.name ?? null}
          version={version}
          zone={mainland.zone}
          onTravel={(zone) => {
            mainland.travel(zone);
            close();
          }}
          onLogout={onLogout}
          onLink={() => setPanel("link")}
          onRestyle={() => setPanel("restyle")}
          onRename={() => setPanel("rename")}
          onClose={close}
        />
      )}
      {panel === "restyle" && character && (
        <RestyleSheet
          character={character}
          inventory={world.inventory}
          priceCents={world.catalog.find((i) => i.id === RESTYLE_TICKET)?.price_usd_cents ?? null}
          paymentsEnabled={paymentsEnabled}
          wallet={wallet}
          onBuy={() => void purchase.start(RESTYLE_TICKET, 1)}
          onLink={() => setPanel("link")}
          onSaved={() => void world.reload()}
          onClose={close}
        />
      )}
      {panel === "rename" && character && (
        <RenameSheet
          character={character}
          inventory={world.inventory}
          priceCents={world.catalog.find((i) => i.id === RENAME_TICKET)?.price_usd_cents ?? null}
          paymentsEnabled={paymentsEnabled}
          wallet={wallet}
          onBuy={() => void purchase.start(RENAME_TICKET, 1)}
          onLink={() => setPanel("link")}
          onSaved={() => void world.reload()}
          onClose={close}
        />
      )}
    </>
  );
}
