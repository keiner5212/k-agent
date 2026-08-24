const STORAGE_KEY = "k-agent.locale";
const DEFAULT = "en";

let locale = localStorage.getItem(STORAGE_KEY) || DEFAULT;

function get(path) {
  return path.split(".").reduce(
    (acc, key) => (acc == null ? undefined : acc[key]),
    window.__messages?.[locale],
  );
}

function apply() {
  document.documentElement.lang = locale;
  for (const el of document.querySelectorAll("[data-i18n]")) {
    const v = get(el.dataset.i18n);
    if (v != null) el.textContent = v;
  }
  for (const el of document.querySelectorAll("[data-locale]")) {
    el.disabled = el.dataset.locale === locale;
  }
}

function setLocale(l) {
  locale = l;
  localStorage.setItem(STORAGE_KEY, l);
  apply();
}

for (const el of document.querySelectorAll("[data-locale]")) {
  el.addEventListener("click", () => setLocale(el.dataset.locale));
}

document.getElementById("health-btn").addEventListener("click", async () => {
  const out = document.getElementById("health-out");
  try {
    const r = await fetch("/api/health");
    const data = await r.json();
    out.textContent = JSON.stringify(data, null, 2);
  } catch (err) {
    out.textContent = String(err);
  }
});

let ws = null;
const wsStatus = document.getElementById("ws-status");
const wsOut = document.getElementById("ws-out");
const wsBtn = document.getElementById("ws-btn");

function setStatus(s) {
  wsStatus.dataset.state = s;
  wsStatus.textContent = get(`ws.status${s[0].toUpperCase()}${s.slice(1)}`) || s;
  wsBtn.textContent = (s === "open" || s === "connecting")
    ? (get("ws.disconnect") || s)
    : (get("ws.connect") || s);
}

wsBtn.addEventListener("click", () => {
  if (ws && ws.readyState <= 1) {
    ws.close();
    return;
  }
  setStatus("connecting");
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  ws = new WebSocket(`${proto}//${location.host}/ws`);
  ws.onopen = () => {
    setStatus("open");
    ws.send(JSON.stringify({ type: "ping" }));
  };
  ws.onmessage = (e) => {
    try {
      wsOut.textContent = JSON.stringify(JSON.parse(e.data), null, 2);
    } catch {
      wsOut.textContent = String(e.data);
    }
  };
  ws.onclose = () => {
    setStatus("disconnected");
    ws = null;
  };
  ws.onerror = () => setStatus("disconnected");
});

apply();
setStatus("disconnected");