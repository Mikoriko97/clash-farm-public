// The HUD (docs/UI.md): corners for chrome, centre for the world, one context
// action where the character stands. Panels switch Godot to `ui` mode.
import { useEffect, useRef, useState } from "react";
import { getAuthApi } from "../api";
import { focusWorld, sendToGodot } from "../bridge/host";
import { accusative } from "../island/labels";
import { useIsland } from "../island/useIsland";
import { useMainland } from "../mainland/useMainland";
import { usePurchase } from "../payments/usePurchase";
import { usePresence } from "../presence/usePresence";
import { useAppStore } from "../store/appStore";
import { loadWalletManager } from "../wallet";
import { Identity, type Panel, Resources, Tools } from "./Chrome";
import { ContextActionButton } from "./ContextAction";
import { Curtain } from "./Curtain";
import { LevelUp } from "./LevelUp";
import { playUi, primeUiSound } from "./sound";
import { TutorialCard, TutorialDone } from "./Tutorial";
import { useTutorial } from "./useTutorial";
import { error, notice, useToast } from "./useToast";
import { GpuWarning } from "./GpuWarning";
import { HudPanels } from "./HudPanels";
import { Joystick } from "./Joystick";
import { JumpButton } from "./JumpButton";
import { useBuildMode } from "./useBuildMode";
import { useClashBalance } from "./useClashBalance";
import { useCoarsePointer } from "./useCoarsePointer";
import { useContextAction } from "./useContextAction";
import { useHouseEntry } from "./useHouseEntry";
import { useHudKeys } from "./useHudKeys";
import { CharacterCreator } from "../character/CharacterCreator";
import { useCharacter } from "../character/useCharacter";
import { useForcedRename } from "../character/useForcedRename";
import { Welcome } from "./Welcome";
import "./hud.css";

export function Hud() {
  const session = useAppStore((s) => s.session);
  const status = useAppStore((s) => s.godotStatus);
  const progress = useAppStore((s) => s.loadProgress);
  const bytes = useAppStore((s) => s.loadBytes);
  const version = useAppStore((s) => s.godotVersion);
  const lastError = useAppStore((s) => s.lastError);
  if (!session) {
    return (
      <div className="hud">
        <Welcome loading={status === "loading" ? { progress, bytes } : null} error={status === "error" ? lastError : null} />
      </div>
    );
  }
  return (
    <>
      <Curtain />
      <SessionHud wallet={session.wallet} version={version} />
    </>
  );
}

/** `wallet` is null for a guest island (0028): the wallet chip offers to link one, CLASH purchases lead there too. */
function SessionHud({ wallet, version }: { wallet: string | null; version: string | null }) {
  const world = useIsland();
  const weather = useAppStore((s) => s.weather);
  const clock = useAppStore((s) => s.serverClock);
  const character = useCharacter();
  const presence = usePresence();
  const mainland = useMainland(world.reload);
  const setSession = useAppStore((s) => s.setSession);
  const walletInfo = useAppStore((s) => s.wallet);
  const coarse = useCoarsePointer();
  const [panel, setPanel] = useState<Panel>("none");
  const tutorial = useTutorial();
  const enterHouse = useHouseEntry(panel, setPanel);
  useForcedRename(character, panel, setPanel);
  const [seed, setSeed] = useState<string | null>(null);
  const purchase = usePurchase(world.reload);
  const clash = useClashBalance(purchase.step.kind === "done" ? purchase.step.outcome.order_id : null);
  // Any open panel or payment step hands the screen to React; Godot ignores world input.
  // No character yet: the creator owns the screen, the world only listens (mode ui, no shortcuts).
  const creating = character === null;
  const uiOpen = panel !== "none" || purchase.step.kind !== "idle" || creating;
  const { building, setBuilding, buildActive, plotItems } = useBuildMode(world, uiOpen);
  const { action, locked, runAction } = useContextAction({ world, mainland, seed, enterHouse, setPanel });
  useHudKeys({ panel, setPanel, buildActive, setBuilding, plotItems, runAction, enabled: !creating });
  // Any HUD button click hands keyboard focus back to the canvas once the click is done, with a click sound.
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.target instanceof Element && e.target.closest(".hud button")) {
        playUi("click");
        window.setTimeout(focusWorld, 0);
      }
    };
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, []);
  useEffect(() => primeUiSound(), []);
  // Panels whoosh open and closed; errors and notices get their own sound.
  const prevPanel = useRef(panel);
  useEffect(() => {
    if (prevPanel.current !== panel) playUi(panel === "none" ? "close" : "open");
    prevPanel.current = panel;
  }, [panel]);
  const seeds = world.inventory.filter((i) => i.item_id.startsWith("seed_") && i.qty > 0);

  const logout = async () => {
    try {
      await getAuthApi().logout();
    } finally {
      setSession(null);
      await (await loadWalletManager()).disconnect();
    }
  };

  // Errors last: when a notice and an error arrive together, the error is the one to read.
  const toast = useToast([notice(world.notice), notice(mainland.notice), error(world.error), error(mainland.error)]);

  if (character === undefined) return null;
  if (creating) {
    return (
      <div className="hud">
        <CharacterCreator />
      </div>
    );
  }

  return (
    <div className="hud">
      {world.island && <Identity island={world.island.island} wallet={wallet} nickname={character.nickname} />}
      <Resources credits={world.credits} clash={clash} walletName={wallet === null ? "Connect wallet" : (walletInfo?.name ?? null)} weather={weather?.state ?? null} peers={presence.connected ? presence.peers : null} rested={world.island?.rested ?? null} clock={clock} onWallet={() => setPanel(wallet === null ? "link" : "menu")} />
      <div className="mid">
        <GpuWarning />
        <LevelUp />
        {tutorial.celebrateSeq > 0 && <TutorialDone key={tutorial.celebrateSeq} />}
        {toast && (
          <div key={toast.id} className={toast.kind === "error" ? "toast err glass" : "toast glass"} role={toast.kind === "error" ? "alert" : "status"}>
            {toast.text}
          </div>
        )}
        {buildActive && building && (
          <div className="hint glass" role="status">
            {building.kind === "plot" ? "Pick a cell on the ground for the plot" : `Place ${accusative(building.item)}: click a spot, R — rotate`}
            {building.kind === "decor" && (
              <button type="button" className="btn" onClick={() => sendToGodot("build_rotate", { delta_deg: 15 })}>
                ↻ Rotate
              </button>
            )}
            <button type="button" className="btn" onClick={() => setBuilding(null)}>
              Cancel
            </button>
          </div>
        )}
        {panel === "none" && !buildActive && (
          <ContextActionButton
            action={action}
            busy={world.busy !== null || locked}
            seeds={seeds}
            selectedSeed={seed}
            onRun={runAction}
            onSelectSeed={setSeed}
            pulse={tutorial.target === "context"}
            onRemove={
              action?.removable && action.plot
                ? () => void world.removePlot(action.plot!.id)
                : action?.removable && action.decorId
                  ? () => void world.removeDecor(action.decorId!)
                  : null
            }
          />
        )}
      </div>
      {coarse && panel === "none" && (
        <>
          <Joystick />
          <JumpButton />
        </>
      )}
      {tutorial.step && panel === "none" && !buildActive && <TutorialCard step={tutorial.step} coarse={coarse} onSkip={tutorial.skip} />}
      <Tools panel={panel} newItems={0} onOpen={setPanel} plotItems={plotItems} building={buildActive} onBuild={() => setBuilding((b) => (b ? null : { kind: "plot" }))} pulse={tutorial.target === "shop" || tutorial.target === "inventory" ? tutorial.target : null} />
      <HudPanels panel={panel} setPanel={setPanel} world={world} mainland={mainland} purchase={purchase} seed={seed} setSeed={setSeed} setBuilding={setBuilding} wallet={wallet} version={version} onLogout={() => void logout()} />
    </div>
  );
}
