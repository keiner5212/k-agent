import { messages } from "./i18n.ts";

export function renderHome(): string {
  const initScript = `window.__messages = ${JSON.stringify(messages)};`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>k-agent</title>
  <link rel="stylesheet" href="/static/styles.css">
</head>
<body>
  <main class="container">
    <header class="page-header">
      <div>
        <h1>k-agent</h1>
        <p class="subtitle" data-i18n="app.subtitle"></p>
      </div>
      <div class="lang-switcher" role="group" aria-label="Language">
        <button data-locale="en">EN</button>
        <button data-locale="es">ES</button>
      </div>
    </header>

    <section>
      <h2 data-i18n="sections.rest"></h2>
      <button id="health-btn" data-i18n="rest.button"></button>
      <pre id="health-out">...</pre>
    </section>

    <section>
      <h2 data-i18n="sections.ws"></h2>
      <button id="ws-btn" data-i18n="ws.connect"></button>
      <p><span class="dot"></span><span data-i18n="ws.status"></span>: <span id="ws-status"></span></p>
      <pre id="ws-out">...</pre>
    </section>
  </main>
  <script>${initScript}</script>
  <script type="module" src="/static/app.js"></script>
</body>
</html>`;
}