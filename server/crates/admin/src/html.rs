//! The few server-rendered pages left outside the admin app (ADR-022): sign-in and
//! errors. No template engine, every dynamic value goes through `esc`; styles and the
//! login script are served from `/admin.css` and `/login.js` so the CSP can stay
//! `script-src 'self'; style-src 'self'` without inline code.

use axum::response::Html;

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn head(title: &str) -> String {
    format!(
        "<!doctype html><html lang=\"uk\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">\
<title>{} · Clash Farm admin</title><link rel=\"stylesheet\" href=\"/admin.css\"></head>",
        esc(title)
    )
}

/// One centred card (login, errors, "build the app"). `body` is escaped HTML.
pub fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        "{h}<body class=\"bare\"><main class=\"card bare-card\"><div class=\"logo\"><span class=\"mark\">CF</span>Clash Farm</div>\
<h1>{t}</h1>{b}</main></body></html>",
        h = head(title),
        t = esc(title),
        b = body
    ))
}

pub const CSS: &str = "*{box-sizing:border-box}body{margin:0;font:15px/1.45 system-ui,'Segoe UI',sans-serif;background:#e9ebe6;color:#172219}\
a{color:#1f5a3a}h1{margin:0 0 12px;font-size:24px;font-weight:700}small{color:#6b756d}code{background:#eef1ec;padding:1px 5px;border-radius:5px;font-size:13px}\
.card{background:#fff;border-radius:20px;padding:22px 24px;box-shadow:0 1px 2px rgba(0,0,0,.04)}\
.logo{display:flex;align-items:center;gap:10px;font-weight:800;font-size:18px;margin-bottom:16px;justify-content:center}\
.mark{display:inline-grid;place-items:center;width:34px;height:34px;border-radius:11px;background:#1f5a3a;color:#fff;font-size:13px}\
button{font:inherit;padding:10px 18px;border-radius:12px;border:1px solid #1f5a3a;background:#1f5a3a;color:#fff;cursor:pointer}\
input{font:inherit;font-size:22px;letter-spacing:8px;text-align:center;width:200px;padding:10px 12px;border-radius:12px;border:1px solid #b9c3ba}\
.err{background:#fde8e6;border:1px solid #e3a09a;padding:10px 14px;border-radius:12px}\
body.bare{display:flex;align-items:center;justify-content:center;min-height:100vh;padding:24px}.bare-card{width:min(440px,92vw);text-align:center}";

/// Phantom (or any `window.solana` wallet) signs the server-built message; the
/// signature travels as hex like the game's own sign-in. `{ totp: true }` from verify
/// swaps the button for the code form; the pending cookie travels by itself.
pub const LOGIN_JS: &str = "(function(){\
const out=document.getElementById('msg');const say=t=>{out.hidden=false;out.textContent=t;};\
const btn=document.getElementById('login');const form=document.getElementById('totp');const code=document.getElementById('code');\
async function login(){const p=window.solana;if(!p){say('Потрібен Solana-гаманець (Phantom) у цьому браузері.');return;}\
try{const r=await p.connect();const wallet='sol:'+r.publicKey.toString();\
const n=await fetch('/login/nonce?wallet='+encodeURIComponent(wallet));if(!n.ok){say('Відмова: '+n.status);return;}\
const j=await n.json();const sig=await p.signMessage(new TextEncoder().encode(j.message),'utf8');\
const bytes=sig.signature||sig;const hex=Array.from(bytes,b=>b.toString(16).padStart(2,'0')).join('');\
const v=await fetch('/login/verify',{method:'POST',headers:{'content-type':'application/json'},\
body:JSON.stringify({wallet:wallet,signature:hex,message:j.message})});\
if(!v.ok){say('Відмова: '+v.status);return;}const vj=await v.json();\
if(vj.totp){out.hidden=true;btn.hidden=true;form.hidden=false;code.focus();return;}\
location.href='/';}catch(e){say('Помилка: '+(e&&e.message?e.message:e));}}\
async function confirm(ev){ev.preventDefault();\
const t=await fetch('/login/totp',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({code:code.value.trim()})});\
if(t.ok){location.href='/';return;}code.value='';code.focus();\
say(t.status===429?'Забагато спроб, зачекай хвилину.':t.status===403?'Код не підійшов або вхід прострочено — спробуй ще або підпиши знову.':'Відмова: '+t.status);}\
btn.addEventListener('click',login);form.addEventListener('submit',confirm);})();";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_every_dangerous_character() {
        assert_eq!(esc("<b a=\"x\">'&"), "&lt;b a=&quot;x&quot;&gt;&#39;&amp;");
    }
}
