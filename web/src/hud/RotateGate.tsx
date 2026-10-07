import { canEnterFullscreen, enterLandscape } from "./orientation";

/** Підказка замість гри, коли телефон тримають вертикально (показує лише CSS `.rotate-gate`: вузький дотиковий екран у портреті). */
export function RotateGate() {
  return (
    <div className="rotate-gate" role="alert">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <rect x="7" y="2.5" width="10" height="19" rx="2.2" />
        <path d="M11 18.5h2" />
      </svg>
      <h2>Rotate your phone</h2>
      <p>Clash Farm plays in landscape. Turn the phone sideways to continue.</p>
      {canEnterFullscreen() && (
        <button type="button" className="btn" onClick={() => void enterLandscape()}>
          Go fullscreen
        </button>
      )}
    </div>
  );
}
