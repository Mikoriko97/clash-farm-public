import { Component, type ErrorInfo, type ReactNode } from "react";

/** Запасний екран, коли інтерфейс упав: без нього помилка рендера залишає порожнечу над грою. Англійською (ADR-024). */
export function ErrorFallback() {
  return (
    <div className="hud">
      <div className="welcome glass" role="alert">
        <h1>Something went wrong</h1>
        <p>The interface hit an error. Your progress is safe: the game state lives on the server.</p>
        <button type="button" className="btn" onClick={() => window.location.reload()}>
          Reload the game
        </button>
      </div>
    </div>
  );
}

interface State {
  failed: boolean;
}

/** Ловить помилки рендера дерева інтерфейсу (не Godot-полотна, воно живе окремо): показує `ErrorFallback` і пише причину в консоль. */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  override state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("[ui] render failed:", error.message, info.componentStack);
  }

  override render(): ReactNode {
    return this.state.failed ? <ErrorFallback /> : this.props.children;
  }
}
