use axum::{extract::Path, response::Html};

pub async fn terminal_page(Path(id): Path<String>) -> Html<String> {
    Html(format!(
        r##"<!doctype html>
<html>
<head>
<meta charset="utf-8"/>
<title>Agent {id}</title>
<style>
  html, body {{ margin: 0; padding: 0; height: 100%; background: #1e1e1e; overflow: hidden; }}
  #terminal {{ height: 100%; }}
  .bar {{ position: fixed; top: 0; left: 0; right: 0; height: 28px; background: #2d2d2d;
          display: flex; align-items: center; padding: 0 12px; font: 12px monospace; color: #888; z-index: 10; }}
  .bar .id {{ color: #6bc; }}
  .bar .status {{ margin-left: auto; }}
  .bar .dot {{ width: 8px; height: 8px; border-radius: 50%; display: inline-block; margin-right: 6px; }}
  .bar .dot.on {{ background: #5b5; }}
  .bar .dot.off {{ background: #b55; }}
  #terminal {{ padding-top: 28px; }}
</style>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.min.css"/>
</head>
<body>
<div class="bar">
  <span class="dot on" id="dot"></span>
  <span class="id">{id}</span>
  <span class="status" id="status">connecting...</span>
</div>
<div id="terminal"></div>

<script>
// Load scripts dynamically to avoid blocking
function loadScript(url) {{
  return new Promise((resolve, reject) => {{
    const s = document.createElement('script');
    s.src = url;
    s.onload = resolve;
    s.onerror = reject;
    document.head.appendChild(s);
  }});
}}

async function init() {{
  const s = document.getElementById('status');
  s.textContent = 'loading terminal...';
  await loadScript('https://cdn.jsdelivr.net/npm/@xterm/xterm@5/lib/xterm.min.js');
  await Promise.all([
    loadScript('https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0/lib/addon-fit.min.js'),
    loadScript('https://cdn.jsdelivr.net/npm/@xterm/addon-unicode11@0/lib/addon-unicode11.min.js'),
    loadScript('https://cdn.jsdelivr.net/npm/@xterm/addon-web-links@0/lib/addon-web-links.min.js'),
  ]);
  s.textContent = 'connecting...';
  start();
}}

function start() {{
const agentId = "{id}";
const term = new window.Terminal({{
  allowProposedApi: true,
  cursorBlink: true,
  fontSize: 14,
  fontFamily: "Menlo, Consolas, 'DejaVu Sans Mono', 'Liberation Mono', monospace",
  theme: {{ background: '#1e1e1e', foreground: '#d4d4d4', cursor: '#d4d4d4' }}
}});
const fit = new window.FitAddon.FitAddon();
const unicode11 = new window.Unicode11Addon.Unicode11Addon();
const webLinks = new window.WebLinksAddon.WebLinksAddon();
term.loadAddon(fit);
term.loadAddon(unicode11);
term.loadAddon(webLinks);
term.unicode.activeVersion = '11';
term.open(document.getElementById('terminal'));
fit.fit();

const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
const params = new URLSearchParams(location.search);
const tokenParam = params.get('token') ? '&token=' + params.get('token') : '';
const wsUrl = proto + '//' + location.host + '/api/agents/' + agentId + '/ws?' + tokenParam;

let ws;
let reconnectDelay = 1000;

function connect() {{
  ws = new WebSocket(wsUrl);
  const statusEl = document.getElementById('status');
  const dot = document.getElementById('dot');

  ws.onopen = () => {{
    statusEl.textContent = 'connected';
    dot.className = 'dot on';
    reconnectDelay = 1000;

    // Send terminal size
    ws.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
  }};

  ws.onmessage = (e) => {{
    try {{
      const frame = JSON.parse(e.data);
      if (frame.type === 'terminal') {{
        const bytes = Uint8Array.from(atob(frame.data), c => c.charCodeAt(0));
        term.write(bytes);
      }} else if (frame.type === 'status') {{
        statusEl.textContent = frame.status + (frame.exit_code !== null && frame.exit_code !== undefined ? ' (exit ' + frame.exit_code + ')' : '');
        if (frame.status !== 'running' && frame.status !== 'starting') {{
          dot.className = 'dot off';
        }}
      }}
    }} catch {{}}
  }};

  ws.onclose = () => {{
    statusEl.textContent = 'disconnected';
    dot.className = 'dot off';
    setTimeout(connect, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, 30000);
  }};

  ws.onerror = () => {{ ws.close(); }};
}}

// Send keystrokes to the agent
term.onData((data) => {{
  if (ws && ws.readyState === WebSocket.OPEN) {{
    ws.send(JSON.stringify({{ type: 'input', data: data }}));
  }}
}});

// Resize
window.addEventListener('resize', () => {{
  fit.fit();
  if (ws && ws.readyState === WebSocket.OPEN) {{
    ws.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
  }}
}});

connect();
}}

init();
</script>
</body>
</html>"##
    ))
}
