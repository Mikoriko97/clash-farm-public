// While a look editor is open, the real character stands close to the camera and wears the edited
// look (Godot: camera_preview + player_look); on close the camera goes back and, for the paid
// restyle, the character puts the saved look back on (`restore`).
import { useEffect } from "react";
import { sendToGodot } from "../bridge/host";
import { useAppStore } from "../store/appStore";
import type { TabId } from "./CreatorPickers";
import { lookColors, type Look } from "./palette";

export function useLookPreview(look: Look | undefined, yaw: number, tab: TabId, restore?: Look) {
  const godotStatus = useAppStore((s) => s.godotStatus);
  useEffect(() => {
    // Hair, face and hats are judged up close; the body from a few steps away.
    if (godotStatus === "ready") sendToGodot("camera_preview", { on: true, yaw_deg: yaw, focus: tab === "hair" || tab === "face" || tab === "hat" ? "head" : "body" });
  }, [godotStatus, yaw, tab]);
  useEffect(() => {
    if (godotStatus === "ready" && look) sendToGodot("player_look", { look: lookColors(look) });
  }, [godotStatus, look]);
  useEffect(
    () => () => {
      sendToGodot("camera_preview", { on: false });
      if (restore) sendToGodot("player_look", { look: lookColors(useAppStore.getState().character?.look ?? restore) });
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only on unmount: the saved look is read from the store then
    [],
  );
}
