// Ambiente para testar a lógica da interface (stores) em Node: carrega o código real com o Vite (SSR) e troca o
// Tauri por um falso. `fakeBackend` imita o motor de áudio de src-tauri: só retoma o que está carregado.
import { createServer } from "vite";

export function installDom(storage = {}) {
  const ls = { ...storage };
  globalThis.localStorage = { getItem: (k) => (k in ls ? ls[k] : null), setItem: (k, v) => { ls[k] = String(v); }, removeItem: (k) => { delete ls[k]; } };
  globalThis.window = globalThis;
  Object.defineProperty(globalThis, "navigator", { value: { userAgent: "node" }, configurable: true });
  globalThis.addEventListener = () => {};
  return ls;
}

/** Tauri falso: `calls` guarda cada comando; `emit` entrega eventos aos `listen` da interface. */
export function installTauri(handlers = {}) {
  const callbacks = new Map(); const listeners = new Map(); let nextId = 1;
  const calls = [];
  globalThis.__TAURI_INTERNALS__ = {
    transformCallback: (cb) => { const id = nextId++; callbacks.set(id, cb); return id; },
    unregisterCallback: (id) => callbacks.delete(id),
    convertFileSrc: (p) => p,
    invoke: async (cmd, args) => {
      if (cmd === "plugin:event|listen") { const l = listeners.get(args.event) ?? []; l.push(args.handler); listeners.set(args.event, l); return args.handler; }
      if (cmd.startsWith("plugin:event|")) return;
      calls.push({ cmd, args });
      return handlers[cmd]?.(args);
    },
  };
  const emit = (event, payload) => { for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload }); };
  return { calls, emit, named: (c) => calls.filter((x) => x.cmd === c) };
}

/**
 * Motor de áudio falso com as regras do real (src-tauri/src/playback.rs + audio.rs):
 *  - a faixa leva `loadMs` para carregar; pausar, retomar ou buscar nesse intervalo vale para quando ela chegar;
 *  - `audio_resume` sem faixa carregada não faz som;
 *  - a posição só anda enquanto toca.
 */
const stoppers = [];
export function fakeEngine(tauri, { loadMs = 250 } = {}) {
  const e = { token: 0, loaded: null, ready: false, playing: false, pos: 0, start: 0, pend: null, plays: [] };
  let last = Date.now();
  stoppers.push(() => { e.ready = false; });
  const timer = setInterval(() => {
    const now = Date.now(), dt = (now - last) / 1000; last = now;
    if (e.ready && e.playing) { e.pos += dt; tauri.emit("audio:position", { token: e.token, position: e.pos, duration: 200 }); }
  }, 100);
  timer.unref?.(); stoppers.push(() => clearInterval(timer));
  const h = {
    audio_play: ({ track, start, autoplay }) => {
      const token = ++e.token; e.loaded = `${track.provider}:${track.id}`; e.start = start; e.ready = false; e.playing = false;
      e.pend = { start, autoplay }; e.plays.push({ key: e.loaded, start, autoplay });
      setTimeout(() => {
        if (token !== e.token) return;
        e.ready = true; e.pos = e.pend.start; e.playing = e.pend.autoplay;
        tauri.emit("audio:loaded", { token, key: e.loaded, source: track.provider, codec: "aac", bitrate: 128000, duration: 200 });
        tauri.emit("audio:state", { token, state: e.playing ? "playing" : "paused" });
      }, loadMs);
      setTimeout(() => tauri.emit("audio:state", { token, state: "loading", key: e.loaded }), 0);
      return token;
    },
    audio_resume: () => { if (!e.ready) { if (e.pend) e.pend.autoplay = true; return; } e.playing = true; tauri.emit("audio:state", { token: e.token, state: "playing" }); },
    audio_pause: () => { if (!e.ready) { if (e.pend) e.pend.autoplay = false; return; } e.playing = false; tauri.emit("audio:state", { token: e.token, state: "paused" }); },
    audio_seek: ({ seconds }) => { if (!e.ready) { if (e.pend) e.pend.start = seconds; return; } e.pos = seconds; },
    audio_stop: () => { e.loaded = null; e.ready = false; e.playing = false; },
  };
  return { e, handlers: h };
}

/** Liga o Tauri falso ao motor falso. */
export function installApp(storage, opts) {
  installDom(storage);
  const tauri = installTauri(); const fake = fakeEngine(tauri, opts);
  const orig = globalThis.__TAURI_INTERNALS__.invoke;
  globalThis.__TAURI_INTERNALS__.invoke = (cmd, args) => (fake.handlers[cmd] ? (tauri.calls.push({ cmd, args }), Promise.resolve(fake.handlers[cmd](args))) : orig(cmd, args));
  return { tauri, fake: fake.e };
}

export async function loadApp(paths) {
  const vite = await createServer({ configFile: false, logLevel: "silent", server: { middlewareMode: true, hmr: false, watch: null }, appType: "custom", optimizeDeps: { noDiscovery: true, include: [] } });
  const mods = {};
  for (const p of paths) mods[p] = await vite.ssrLoadModule(p);
  // depois de fechar, chamadas tardias do código carregado (ex.: gravar configuração) acham o Vite fechado: não é falha do teste
  return { mods, close: async () => { stoppers.splice(0).forEach((f) => f()); await sleep(1300); await vite.close(); } };
}
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export async function until(what, ok, ms = 3000) {
  const t0 = Date.now();
  while (!ok()) { if (Date.now() - t0 > ms) throw new Error(`tempo esgotado: ${what}`); await sleep(10); }
}
