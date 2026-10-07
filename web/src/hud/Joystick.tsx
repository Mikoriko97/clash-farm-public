import { useEffect, useRef, useState } from "react";
import { sendToGodot } from "../bridge/host";
import { type StickVector, stickVector, travelRadius } from "./stick";

const SEND_EVERY_MS = 50;
/** Поки палець на стіку, поточний вектор повторюється: Godot міг скинути його (вставання з лавки, модалка), а нерухомий палець подій не дає. */
const REPEAT_MS = 100;

/** Touch joystick: pointer offset → `move_input` vector (y < 0 = forward). Zero on release. At the outer edge of the circle
 *  the character runs (Godot decides, `StickRun`); the knob lights up as soon as it gets there. */
export function Joystick() {
  const pad = useRef<HTMLDivElement>(null);
  const knob = useRef<HTMLElement>(null);
  const last = useRef(0);
  const held = useRef<StickVector | null>(null);
  const [run, setRun] = useState(false);

  useEffect(() => {
    const id = window.setInterval(() => {
      const v = held.current;
      if (v) sendToGodot("move_input", { x: v.x, y: v.y });
    }, REPEAT_MS);
    return () => window.clearInterval(id);
  }, []);

  const send = (x: number, y: number, force = false) => {
    const now = performance.now();
    if (!force && now - last.current < SEND_EVERY_MS) return;
    last.current = now;
    sendToGodot("move_input", { x: Number(x.toFixed(2)), y: Number(y.toFixed(2)) });
  };

  const move = (e: React.PointerEvent) => {
    const el = pad.current;
    if (!el || !(e.buttons & 1) && e.pointerType !== "touch") return;
    const r = el.getBoundingClientRect();
    const radius = travelRadius(r.width, knob.current?.offsetWidth ?? 0);
    const dx = e.clientX - (r.left + r.width / 2);
    const dy = e.clientY - (r.top + r.height / 2);
    const v = stickVector(dx, dy, radius);
    if (knob.current) knob.current.style.transform = `translate(${v.x * radius}px, ${v.y * radius}px)`;
    setRun(v.run);
    held.current = v;
    send(v.x, v.y);
  };

  const release = () => {
    if (knob.current) knob.current.style.transform = "";
    setRun(false);
    held.current = null;
    send(0, 0, true);
  };

  return (
    <div className="move">
      <div
        ref={pad}
        className={run ? "stick glass run" : "stick glass"}
        role="slider"
        aria-label="Move character"
        aria-valuenow={0}
        tabIndex={-1}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          move(e);
        }}
        onPointerMove={move}
        onPointerUp={release}
        onPointerCancel={release}
      >
        <i ref={knob} />
      </div>
    </div>
  );
}
