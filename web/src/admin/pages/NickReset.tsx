// Скидання ніка з картки гравця (0031, ADR-040): причина обовʼязкова, у гравця — тимчасове
// ім'я Farmer_… і одна безкоштовна зміна, старе ім'я (і двійники) блокується для всіх.
import { useState } from "react";
import { api } from "../api";
import { navigate } from "../router";

export function NickReset({ id, nickname, onDone }: { id: string; nickname: string | null; onDone: () => void }) {
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ text: string; bad: boolean } | null>(null);
  if (!nickname) return <p className="muted">Персонажа ще нема — скидати нічого.</p>;
  const run = async () => {
    setBusy(true);
    setMsg(null);
    try {
      const done = await api.resetNickname(id, nickname, reason);
      setMsg({ text: done.repeated ? `Уже скинуто: ${done.old} → ${done.new}.` : `Скинуто: ${done.old} → ${done.new}. Ім'я заблоковане.`, bad: false });
      setReason("");
      onDone();
    } catch (e) {
      setMsg({ text: e instanceof Error ? e.message : "Помилка", bad: true });
    } finally {
      setBusy(false);
    }
  };
  return (
    <form onSubmit={(e) => { e.preventDefault(); void run(); }}>
      <p className="muted">
        Зараз: <b>{nickname}</b>. Гравець отримає тимчасове ім'я і одну безкоштовну зміну; старе ім'я заблокується разом зі схожими написаннями.{" "}
        <a href="/names" onClick={(e) => { e.preventDefault(); navigate({ page: "names" }); }}>Усі блоки</a>
      </p>
      <p><input id="nick-reset-reason" type="text" value={reason} maxLength={200} placeholder="Причина скидання" onChange={(e) => setReason(e.target.value)} /></p>
      {msg && <p className={msg.bad ? "alert crit" : "muted"}>{msg.text}</p>}
      <button type="submit" className="small danger" disabled={busy || reason.trim() === ""}>Скинути нік</button>
    </form>
  );
}
