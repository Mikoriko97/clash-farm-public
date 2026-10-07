// Small building blocks of every admin page. No inline styles: cf-admin's CSP is
// `style-src 'self'`, so sizes live in admin.css and data-driven shapes are SVG attributes.
import type { ReactNode } from "react";

export function Title({ title, subtitle, right }: { title: string; subtitle?: string; right?: ReactNode }) {
  return (
    <div className="title">
      <div>
        <h1>{title}</h1>
        {subtitle && <p>{subtitle}</p>}
      </div>
      {right && <div className="title-right">{right}</div>}
    </div>
  );
}

export function Card({ title, children, className = "", right }: { title?: string; children: ReactNode; className?: string; right?: ReactNode }) {
  return (
    <section className={`card ${className}`}>
      {(title || right) && (
        <header className="card-head">
          {title && <h2>{title}</h2>}
          {right}
        </header>
      )}
      {children}
    </section>
  );
}

export function Stat({ label, value, note, tone = "" }: { label: string; value: string; note?: string; tone?: "" | "dark" | "warn" | "crit" }) {
  return (
    <div className={`card stat ${tone}`}>
      <small>{label}</small>
      <span className="n">{value}</span>
      {note && <span className="d">{note}</span>}
    </div>
  );
}

export function Tag({ children, tone = "" }: { children: ReactNode; tone?: "" | "on" | "off" | "warn" | "info" }) {
  return <span className={`tag ${tone}`}>{children}</span>;
}

export function Empty({ text }: { text: string }) {
  return <p className="muted empty">{text}</p>;
}

export function Loading() {
  return <p className="muted">Завантаження…</p>;
}

export function ErrorBox({ text }: { text: string }) {
  return <p className="alert crit">{text}</p>;
}
