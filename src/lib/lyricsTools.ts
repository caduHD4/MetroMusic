// Letras escolhidas ou escritas à mão, leitura/escrita de LRC e a imagem para compartilhar.
import type { LyricLine, Lyrics } from "./catalog";

// ───────── LRC

const STAMP = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g;
const WORD_TAG = /<\d{1,3}:\d{2}(?:[.:]\d{1,3})?>/g;

/** Reparte o tempo da linha entre as palavras, proporcional ao tamanho (mesma regra do backend). */
export function spreadWords(text: string, t: number, end: number): LyricLine["words"] {
  const ws = text.split(/\s+/).filter(Boolean);
  const total = Math.max(1, ws.reduce((n, w) => n + [...w].length + 1, 0));
  let at = t;
  return ws.map((w) => { const d = (end - t) * ([...w].length + 1) / total; const o = { t: at, end: at + d, w }; at += d; return o; });
}

/** Lê LRC (`[mm:ss.xx] texto`). Sem nenhuma marca de tempo, vira letra não sincronizada. */
export function parseLrc(text: string, provider: string): Lyrics | null {
  const raw: { t: number; text: string }[] = [];
  for (const line of text.split(/\r?\n/)) {
    const stamps = [...line.matchAll(STAMP)].map((m) => {
      const frac = m[3] ? Number(m[3]) / 10 ** m[3].length : 0;
      return Number(m[1]) * 60 + Number(m[2]) + frac;
    });
    if (!stamps.length) continue;
    const body = line.replace(STAMP, "").replace(WORD_TAG, "").trim();
    for (const t of stamps) raw.push({ t, text: body });
  }
  if (raw.length) {
    raw.sort((a, b) => a.t - b.t);
    const lines: LyricLine[] = raw.map((r, i) => {
      const next = raw[i + 1]?.t ?? r.t + 5;
      const end = Math.max(r.t + 0.3, Math.min(r.t + 7, Math.max(r.t + 0.6, next - 0.08)));
      return { t: r.t, end, text: r.text, words: spreadWords(r.text, r.t, end) };
    }).filter((l) => l.text);
    if (lines.length) return { synced: true, provider, lines };
  }
  const plain = text.split(/\r?\n/).map((l) => l.trim()).filter((l) => l && !/^\[[a-z]{2,}:.*\]$/i.test(l));
  return plain.length ? { synced: false, provider, lines: plain.map((l) => ({ t: 0, end: 0, text: l, words: [] })) } : null;
}

export function stamp(sec: number): string {
  const cs = Math.max(0, Math.round(sec * 100));
  const m = Math.floor(cs / 6000), s = Math.floor((cs % 6000) / 100), c = cs % 100;
  return `[${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(c).padStart(2, "0")}]`;
}

/** Texto para o editor: LRC se for sincronizada, linhas simples se não. */
export function toLrc(l: Lyrics): string {
  return l.lines.map((x) => (l.synced ? `${stamp(x.t)} ${x.text}` : x.text)).join("\n");
}

// ───────── Letras trocadas pela pessoa (persistem por faixa)

const KEY = "metromusic-lyrics-overrides-v1";
type Store = Record<string, Lyrics>;
let cache: Store | null = null;
function load(): Store { if (!cache) { try { cache = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Store; } catch { cache = {}; } } return cache; }
function save() { try { localStorage.setItem(KEY, JSON.stringify(cache ?? {})); } catch { /* sem armazenamento: vale só nesta sessão */ } }

export const getOverride = (id: string): Lyrics | null => load()[id] ?? null;
export function setOverride(id: string, l: Lyrics) { load()[id] = l; save(); }
export function clearOverride(id: string) { delete load()[id]; save(); }

// ───────── Ajuste de sincronia (persiste por faixa)

const OFFSET_KEY = "metromusic-lyrics-offsets-v1";
export function loadOffsets(): Record<string, number> { try { return JSON.parse(localStorage.getItem(OFFSET_KEY) ?? "{}"); } catch { return {}; } }
export function saveOffsets(o: Record<string, number>) { try { localStorage.setItem(OFFSET_KEY, JSON.stringify(o)); } catch { /* idem */ } }

// ───────── Imagem da letra

export type ImageStyle = "cover" | "dark" | "gradient";

function loadImage(src: string): Promise<HTMLImageElement | null> {
  return new Promise((res) => { const i = new Image(); i.crossOrigin = "anonymous"; i.onload = () => res(i); i.onerror = () => res(null); i.src = src; });
}

function wrap(ctx: CanvasRenderingContext2D, text: string, maxW: number): string[] {
  const out: string[] = []; let cur = "";
  for (const w of text.split(/\s+/)) {
    const test = cur ? `${cur} ${w}` : w;
    if (cur && ctx.measureText(test).width > maxW) { out.push(cur); cur = w; } else cur = test;
  }
  if (cur) out.push(cur);
  return out;
}

/** Desenha o cartão 1080×1350 (4:5) com capa, título, artista e as linhas escolhidas. */
export async function renderLyricsImage(o: { title: string; artist: string; art?: string; hue: number; lines: string[]; style: ImageStyle }): Promise<Blob> {
  const W = 1080, H = 1350, P = 84;
  const c = document.createElement("canvas"); c.width = W; c.height = H;
  const ctx = c.getContext("2d")!;
  const art = o.art ? await loadImage(o.art) : null;
  if (o.style === "dark") { ctx.fillStyle = "#15131c"; ctx.fillRect(0, 0, W, H); }
  else if (o.style === "gradient" || !art) {
    const g = ctx.createLinearGradient(0, 0, W, H);
    g.addColorStop(0, `oklch(0.6 0.17 ${o.hue})`); g.addColorStop(1, `oklch(0.35 0.13 ${(o.hue + 50) % 360})`);
    ctx.fillStyle = g; ctx.fillRect(0, 0, W, H);
  } else {
    ctx.filter = "blur(60px) saturate(1.4)"; ctx.drawImage(art, -120, -120, W + 240, H + 240); ctx.filter = "none";
    ctx.fillStyle = "rgba(0,0,0,.5)"; ctx.fillRect(0, 0, W, H);
  }
  ctx.fillStyle = "#fff"; ctx.textBaseline = "top";
  const sz = 120;
  if (art) { ctx.save(); ctx.beginPath(); ctx.roundRect(P, P, sz, sz, 18); ctx.clip(); ctx.drawImage(art, P, P, sz, sz); ctx.restore(); }
  const tx = P + (art ? sz + 28 : 0), tw = W - tx - P;
  ctx.font = "700 40px system-ui, sans-serif"; ctx.fillText(fit(ctx, o.title, tw), tx, P + 14);
  ctx.fillStyle = "rgba(255,255,255,.72)"; ctx.font = "500 32px system-ui, sans-serif"; ctx.fillText(fit(ctx, o.artist, tw), tx, P + 70);
  // letra: maior tamanho que couber
  ctx.fillStyle = "#fff";
  const area = { y: P + sz + 90, h: H - (P + sz + 90) - 170 };
  let font = 84, rows: string[] = [];
  for (; font >= 38; font -= 4) {
    ctx.font = `800 ${font}px system-ui, sans-serif`;
    rows = o.lines.flatMap((l) => wrap(ctx, l, W - P * 2));
    if (rows.length * font * 1.2 <= area.h) break;
  }
  rows.forEach((r, i) => ctx.fillText(r, P, area.y + i * font * 1.2));
  ctx.fillStyle = "rgba(255,255,255,.6)"; ctx.font = "700 28px system-ui, sans-serif"; ctx.fillText("METROMUSIC", P, H - P - 28);
  return new Promise((res, rej) => c.toBlob((b) => (b ? res(b) : rej(new Error("não consegui gerar a imagem"))), "image/png"));
}

function fit(ctx: CanvasRenderingContext2D, s: string, maxW: number): string {
  if (ctx.measureText(s).width <= maxW) return s;
  let t = s; while (t.length > 1 && ctx.measureText(`${t}…`).width > maxW) t = t.slice(0, -1);
  return `${t}…`;
}

export async function blobToBase64(b: Blob): Promise<string> {
  const buf = new Uint8Array(await b.arrayBuffer()); let s = "";
  for (let i = 0; i < buf.length; i += 0x8000) s += String.fromCharCode(...buf.subarray(i, i + 0x8000));
  return btoa(s);
}
