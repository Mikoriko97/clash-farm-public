import { GodotCanvas } from "./bridge/GodotCanvas";
import { ErrorBoundary } from "./hud/ErrorBoundary";
import { Hud } from "./hud/Hud";
import { RotateGate } from "./hud/RotateGate";
import "./app.css";

export function App() {
  return (
    <>
      <GodotCanvas />
      <ErrorBoundary>
        <Hud />
      </ErrorBoundary>
      <RotateGate />
    </>
  );
}
