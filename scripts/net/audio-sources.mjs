#!/usr/bin/env node
// Réplica do "Choose audio source" do MetroFusePlus (ProviderMatchSearch.search, perProviderLimit = 6)
// + tentativa de reprodução de cada candidato, do mesmo jeito que cada *AudioProvider.resolve faz.
//
// Uso:  node --openssl-legacy-provider scripts/net/audio-sources.mjs [videoId] [--no-play] [--json]
// Env:  DEEZER_ARL (cookie arl p/ o caminho "conta"), QOBUZ_INSTANCES (separados por vírgula),
//       DEEZER_QUALITY (MP3_128|MP3_320|FLAC), TITLE/ARTIST/ALBUM/DURATION
import crypto from "node:crypto";

const args = process.argv.slice(2);
const VIDEO_ID = args.find((a) => !a.startsWith("--")) ?? "Kr4EQDVETuA";
const PLAY = !args.includes("--no-play");
const LIMIT = 6; // perProviderLimit do diálogo
const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const env = process.env;
const list = (v, d) => (v ? v.split(/[\s,;]+/).filter(Boolean) : d).map((u) => u.replace(/\/+$/, ""));
const QOBUZ_INSTANCES = list(env.QOBUZ_INSTANCES, ["https://kenny-api.hf.space"]); // no app o padrão é vazio (só o health checker usa este)
const DEEZER_RESOLVER = "https://yesitworkssomehow-funny-deeza-api-and-yeah.hf.space/get_url";
const DEEZER_RESOLVER_128 = "https://api-number-2-poopo.onrender.com/get_url";
const DEEZER_RESOLVER_RENDER = "https://dzmedia-metrofuse.onrender.com/get_url";
const DEEZER_QUALITY = env.DEEZER_QUALITY ?? "MP3_128";

async function http(url, { timeout = 20000, ...init } = {}) {
  const res = await fetch(url, { ...init, signal: AbortSignal.timeout(timeout) });
  return res;
}
async function json(url, init) {
  const res = await http(url, init);
  const text = await res.text();
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${text.slice(0, 160)}`);
  return JSON.parse(text);
}
const qs = (o) => new URLSearchParams(o).toString();

// ---------- normalização dos termos de busca ----------
const norm = (s) => (s ?? "").toLowerCase().normalize("NFD").replace(/\p{Mn}+/gu, "").replace(/[^a-z0-9]+/g, " ").trim();
const titleNorm = (s) => norm(s).replace(/\b(feat|ft|featuring)\b.*$/, "").replace(/\b(explicit|clean|remaster|remastered|version|audio|official)\b/g, " ").replace(/\s+/g, " ").trim();
const queryTitle = (s) => s.trim().replace(/\s*[\[(]\s*(feat\.?|ft\.?|featuring)\b.*?[\])]/gi, "").replace(/\s*-\s*(explicit|clean|remaster(?:ed)?|audio|official)\b.*$/gi, "").replace(/\s+/g, " ").trim();
const queryArtist = (s) => s.trim().split(",")[0].replace(/\s+/g, " ").trim();
const uniq = (a) => [...new Set(a.map((s) => s.trim()).filter(Boolean))];

// ---------- metadados do vídeo (o app já tem isso do YouTube Music) ----------
// Cadeia do YTPlayerUtils (MAIN_CLIENT WEB_REMIX + STREAM_FALLBACK_CLIENTS). Fora daqui: WEB_REMIX/WEB/WEB_CREATOR
// (precisam de decifrar assinatura + n-transform + PoToken) e TVHTML5 (loginRequired).
const AVR = (v, cronet, model = "Quest 3") => ({ clientName: "ANDROID_VR", clientVersion: v, id: "28", osName: "Android", osVersion: "12", deviceMake: "Oculus", deviceModel: "Quest 3", androidSdkVersion: "32",
  ua: `com.google.android.apps.youtube.vr.oculus/${v} (Linux; U; Android 12; en_US; ${model}; Build/SQ3A.220605.009.A1; Cronet/${cronet})` });
const YT_CLIENTS = [
  { clientName: "TVHTML5_SIMPLY_EMBEDDED_PLAYER", clientVersion: "2.0", id: "85", embedded: true, ua: "Mozilla/5.0 (PlayStation; PlayStation 4/12.02) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.4 Safari/605.1.15" },
  { label: "ANDROID_VR 1.43", ...AVR("1.43.32", "107.0.5284.2") },
  { label: "ANDROID_VR 1.61", ...AVR("1.61.48", "132.0.6808.3") },
  { clientName: "ANDROID_CREATOR", clientVersion: "25.03.101", id: "14", osName: "Android", osVersion: "15", deviceMake: "Google", deviceModel: "Pixel 9 Pro Fold", androidSdkVersion: "35",
    ua: "com.google.android.apps.youtube.creator/25.03.101 (Linux; U; Android 15; en_US; Pixel 9 Pro Fold; Build/AP3A.241005.015.A2; Cronet/132.0.6779.0)" },
  { label: "IPADOS", clientName: "IOS", clientVersion: "21.03.3", id: "5", osName: "iPadOS", osVersion: "17.7.10.21H450", deviceMake: "Apple", deviceModel: "iPad7,6", ua: "com.google.ios.youtube/21.03.3 (iPad7,6; U; CPU iPadOS 17_7_10 like Mac OS X; en-US)" },
  { label: "ANDROID_VR no-auth", ...AVR("1.61.48", "132.0.6808.3", "Oculus Quest 3"), osName: undefined, osVersion: undefined, deviceMake: undefined, deviceModel: undefined, androidSdkVersion: undefined },
  { label: "ANDROID (MOBILE)", clientName: "ANDROID", clientVersion: "21.03.38", id: "3", ua: "com.google.android.youtube/21.03.38 (Linux; U; Android 14) gzip" },
  { clientName: "IOS", clientVersion: "19.45.4", id: "5", osVersion: "18.1.1.22B91", ua: "com.google.ios.youtube/19.45.4 (iPhone16,2; U; CPU iOS 18_1 like Mac OS X; en_US)" },
];
let visitorP;
const visitorData = () => (visitorP ??= http("https://music.youtube.com/", { headers: { "User-Agent": UA, Cookie: "SOCS=CAI" } }).then((r) => r.text()).then((h) => h.match(/"VISITOR_DATA":"([^"]+)"/)?.[1] ?? null).catch(() => null));
async function ytPlayerWith(c, videoId) {
  const { label, id, ua, embedded, ...client } = c;
  const vd = await visitorData();
  return json("https://music.youtube.com/youtubei/v1/player?prettyPrint=false", {
    method: "POST",
    headers: { "Content-Type": "application/json", "User-Agent": ua, "X-Youtube-Client-Name": id, "X-Youtube-Client-Version": c.clientVersion, Origin: "https://music.youtube.com", Referer: "https://music.youtube.com/", ...(vd ? { "X-Goog-Visitor-Id": vd } : {}) },
    body: JSON.stringify({ context: { client: { ...client, hl: "en", gl: "US", visitorData: vd ?? undefined }, ...(embedded ? { thirdParty: { embedUrl: `https://www.youtube.com/watch?v=${videoId}` } } : {}) }, videoId, contentCheckOk: true, racyCheckOk: true }),
  });
}
// devolve o 1º cliente com status OK e formato de áudio com url direta; `log` guarda o que cada cliente respondeu
async function ytResolve(videoId) {
  const log = [];
  for (const c of YT_CLIENTS) {
    const name = c.label ?? c.clientName;
    try {
      const p = await ytPlayerWith(c, videoId);
      const st = p.playabilityStatus?.status;
      if (st !== "OK") { log.push(`${name}: ${st} ${(p.playabilityStatus?.reason ?? "").slice(0, 40)}`); continue; }
      const f = (p.streamingData?.adaptiveFormats ?? []).filter((x) => x.mimeType?.startsWith("audio/") && x.url).sort((a, b) => b.bitrate - a.bitrate)[0];
      if (!f) { log.push(`${name}: OK mas sem url direta (assinatura cifrada)`); continue; }
      return { client: name, ua: c.ua, format: f, details: p.videoDetails, log };
    } catch (e) { log.push(`${name}: ${e.message.slice(0, 60)}`); }
  }
  throw new Error(log.join(" | "));
}

async function ytmSearch(term) {
  const root = await json("https://music.youtube.com/youtubei/v1/search?prettyPrint=false", {
    method: "POST",
    headers: { "Content-Type": "application/json", "User-Agent": UA, Origin: "https://music.youtube.com" },
    body: JSON.stringify({ context: { client: { clientName: "WEB_REMIX", clientVersion: "1.20250101.01.00", hl: "en", gl: "US" } }, query: term, params: "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D" }), // FILTER_SONG
  });
  const sections = root.contents?.tabbedSearchResultsRenderer?.tabs?.[0]?.tabRenderer?.content?.sectionListRenderer?.contents ?? [];
  const out = [];
  for (const s of sections) for (const it of s.musicShelfRenderer?.contents ?? []) {
    const r = it.musicResponsiveListItemRenderer;
    const id = r?.playlistItemData?.videoId;
    if (!id) continue;
    const col = (i) => r.flexColumns?.[i]?.musicResponsiveListItemFlexColumnRenderer?.text?.runs ?? [];
    const sub = col(1);
    const artists = sub.filter((x) => x.navigationEndpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType === "MUSIC_PAGE_TYPE_ARTIST").map((x) => x.text);
    const album = sub.find((x) => x.navigationEndpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType === "MUSIC_PAGE_TYPE_ALBUM")?.text ?? null;
    const dur = sub.map((x) => x.text).find((t) => /^\d+(:\d{2})+$/.test(t));
    const durationMs = dur ? dur.split(":").reduce((a, n) => a * 60 + +n, 0) * 1000 : null;
    out.push({ id, title: col(0)[0]?.text ?? "", artist: artists.join(", ") || sub[0]?.text || "", album, durationMs });
  }
  return out;
}

async function loadMetadata() {
  const meta = { id: VIDEO_ID, title: env.TITLE, artists: env.ARTIST ? [env.ARTIST] : null, album: env.ALBUM ?? null, durationMs: env.DURATION ? +env.DURATION * 1000 : null };
  if (!meta.title || !meta.artists) {
    try {
      const d = (await ytResolve(VIDEO_ID)).details;
      meta.title ??= d.title;
      meta.artists ??= [d.author.replace(/ - Topic$/, "")];
      meta.durationMs ??= +d.lengthSeconds * 1000;
    } catch (e) { console.error("! metadados via player falharam:", e.message); }
  }
  if (VIDEO_ID === "Kr4EQDVETuA") { meta.title ??= "Billie Jean"; meta.artists ??= ["Michael Jackson"]; meta.durationMs ??= 294000; }
  if (!meta.album && meta.title) {
    const hit = (await ytmSearch(`${meta.title} ${meta.artists[0]}`).catch(() => [])).find((x) => x.id === VIDEO_ID);
    if (hit) { meta.album = hit.album; meta.artists = hit.artist ? hit.artist.split(", ") : meta.artists; }
  }
  return meta;
}
// MediaMetadata.searchTerm(): título + 1º artista + álbum
const searchTerm = (m) => [m.title, m.artists[0], m.album].filter(Boolean).join(" ");

// ---------- sonda de reprodução ----------
function magic(buf) {
  const b = Buffer.from(buf);
  if (b.subarray(0, 4).toString() === "fLaC") return "FLAC";
  if (b.subarray(0, 3).toString() === "ID3") return "MP3 (ID3)";
  if (b[0] === 0xff && (b[1] & 0xe0) === 0xe0) return (b[1] & 0x06) === 0 ? "AAC (ADTS)" : "MP3";
  if (b.subarray(4, 8).toString() === "ftyp") return `MP4 (${b.subarray(8, 12).toString()})`;
  if (b.subarray(4, 8).toString() === "styp" || b.subarray(4, 8).toString() === "moof") return "fMP4 segment";
  if (b.subarray(0, 4).toString("hex") === "1a45dfa3") return "WebM";
  if (b.subarray(0, 4).toString() === "OggS") return "Ogg";
  if (b[0] === 0x47) return "MPEG-TS";
  return null;
}
async function probe(url, headers = {}, transform) {
  const res = await http(url, { headers: { Range: "bytes=0-4095", "User-Agent": UA, ...headers } });
  if (!res.ok) throw new Error(`stream HTTP ${res.status}`);
  let buf = Buffer.from(await res.arrayBuffer());
  if (transform) buf = transform(buf);
  const kind = magic(buf);
  const total = res.headers.get("content-range")?.split("/")[1] ?? res.headers.get("content-length");
  if (!kind) throw new Error(`bytes não reconhecidos (${res.headers.get("content-type")}, ${buf.subarray(0, 8).toString("hex")})`);
  return `${kind}${total && total !== "*" ? `, ${(total / 1048576).toFixed(1)} MB` : ""}`;
}
async function probeHls(url, headers = {}) {
  const res = await http(url, { headers: { "User-Agent": UA, ...headers } });
  if (!res.ok) throw new Error(`HLS HTTP ${res.status}`);
  const text = await res.text();
  const map = text.match(/#EXT-X-MAP:URI="([^"]+)"/)?.[1];
  const seg = map ?? text.split("\n").find((l) => l.trim() && !l.startsWith("#"));
  if (!seg) throw new Error("playlist HLS vazia");
  return `HLS → ${await probe(new URL(seg.trim(), url).toString(), headers)}`;
}

// ---------- SoundCloud ----------
let scClientId;
const SC_H = { "User-Agent": UA, Referer: "https://soundcloud.com/", Origin: "https://soundcloud.com", Accept: "application/json" };
async function scClient() {
  if (scClientId) return scClientId;
  const html = await (await http("https://soundcloud.com", { headers: { "User-Agent": UA } })).text();
  const re = /client_id\s*[:=]\s*"([A-Za-z0-9]{32})"/;
  scClientId = html.match(re)?.[1];
  if (!scClientId) {
    const scripts = [...new Set(html.match(/https:\/\/[a-z0-9-]+\.sndcdn\.com\/[^"'<>!]+\.js/g) ?? [])];
    const bodies = await Promise.all(scripts.map((u) => http(u, { headers: { "User-Agent": UA } }).then((r) => r.text()).catch(() => "")));
    scClientId = bodies.map((b) => b.match(re)?.[1]).find(Boolean);
  }
  if (!scClientId) throw new Error("client_id do SoundCloud não encontrado");
  return scClientId;
}
async function scSearch(meta) {
  const cid = await scClient();
  const root = await json(`https://api-v2.soundcloud.com/search/tracks?${qs({ q: searchTerm(meta), limit: LIMIT, client_id: cid, app_locale: "en" })}`, { headers: SC_H });
  return (root.collection ?? []).filter((t) => t.kind === "track" && t.streamable !== false).map((t) => ({
    provider: "SoundCloud", trackId: t.permalink_url, title: t.title, artist: t.publisher_metadata?.artist || t.user?.username || "", album: null, durationMs: t.full_duration ?? t.duration, raw: t,
  }));
}
async function scPlay(c) {
  const cid = await scClient();
  const t = c.raw.media?.transcodings ? c.raw : await json(`https://api-v2.soundcloud.com/tracks/${c.raw.id}?${qs({ client_id: cid, app_locale: "en" })}`, { headers: SC_H });
  // selectStreamCandidate com AAC_160: bitrate/mime alvo → aac → hls → maior bitrate
  const cands = (t.media?.transcodings ?? []).filter((x) => ["progressive", "hls"].includes(x.format?.protocol) && /mpeg|mp3|aac|mp4/.test(x.format?.mime_type ?? "") && x.url)
    .map((x) => ({ ...x, hls: x.format.protocol === "hls", mime: x.format.mime_type.toLowerCase(), br: /160/.test(x.preset) ? 160000 : /96/.test(x.preset) ? 96000 : /mp3|128/.test(x.preset) ? 128000 : 0 }));
  const rank = (x) => [x.br === 160000 ? (x.mime.includes("aac") ? 400 : 300) : 0, /aac|mp4a/.test(x.mime) ? 200 : 0, x.hls ? 150 : 0, x.br];
  cands.sort((a, b) => { const ra = rank(a), rb = rank(b); for (let i = 0; i < ra.length; i++) if (ra[i] !== rb[i]) return rb[i] - ra[i]; return 0; });
  const pick = cands.find((x) => !x.snipped && !/encrypted/.test(x.format.protocol)) ?? cands[0];
  if (!pick) throw new Error("nenhum transcoding suportado");
  const u = new URL(pick.url);
  u.searchParams.set("client_id", cid);
  if (t.track_authorization) u.searchParams.set("track_authorization", t.track_authorization);
  const stream = (await json(u, { headers: SC_H })).url;
  const how = `${pick.preset} ${pick.format.protocol}${pick.snipped ? " [PRÉVIA 30s]" : ""}`;
  return `${how} → ${pick.hls ? await probeHls(stream, SC_H) : await probe(stream, SC_H)}`;
}

// ---------- Deezer ----------
function deezerTerms(meta) {
  const base = queryTitle(meta.title);
  const titles = uniq([base, titleNorm(base), base.replace(/[\[(]([^)\]]+)[)\]]/g, " $1 "), base.replace(/[\[(][^)\]]+[)\]]/g, " ")].map((s) => s.replace(/\s+/g, " ")));
  const primary = queryArtist(meta.artists[0] ?? ""), all = meta.artists.map(queryArtist).join(" "), album = meta.album ? queryTitle(meta.album) : "";
  return uniq(titles.flatMap((t) => [[t, primary], [t, all], [t, primary, album], [t]].map((p) => p.filter(Boolean).join(" "))));
}
async function deezerSearch(meta) {
  const results = new Map();
  for (const term of deezerTerms(meta)) {
    const root = await json(`https://api.deezer.com/search/track?${qs({ q: term, limit: 12 })}`, { headers: { Accept: "application/json", "User-Agent": UA } }).catch(() => null);
    for (const t of root?.data ?? []) {
      if (!results.has(String(t.id))) results.set(String(t.id), { provider: "Deezer", trackId: String(t.id), title: t.title, artist: t.artist?.name ?? "", album: t.album?.title ?? null, durationMs: t.duration ? t.duration * 1000 : null });
      if (results.size >= LIMIT) return [...results.values()];
    }
  }
  return [...results.values()];
}
function deezerDecryptFirstBlocks(trackId) {
  const md5 = crypto.createHash("md5").update(trackId).digest("hex"), secret = "g4el58wc0zvf9na1";
  const key = Buffer.from(Array.from({ length: 16 }, (_, i) => md5.charCodeAt(i) ^ md5.charCodeAt(i + 16) ^ secret.charCodeAt(i)));
  return (buf) => {
    const out = Buffer.from(buf);
    for (let i = 0, off = 0; off + 2048 <= buf.length; i++, off += 2048) {
      if (i % 3) continue; // BF_CBC_STRIPE: só 1 a cada 3 blocos de 2048 bytes é cifrado
      const d = crypto.createDecipheriv("bf-cbc", key, Buffer.from([0, 1, 2, 3, 4, 5, 6, 7]));
      d.setAutoPadding(false);
      Buffer.concat([d.update(buf.subarray(off, off + 2048)), d.final()]).copy(out, off);
    }
    return out;
  };
}
function deezerEnvelope(root, preferred) {
  const medias = root.data?.[0]?.media ?? [];
  const media = medias.find((m) => (m.format ?? "").toUpperCase() === preferred) ?? medias[0];
  if (!media) throw new Error(`sem media (${JSON.stringify(root.data?.[0]?.errors ?? root.errors ?? root).slice(0, 140)})`);
  const src = media.sources?.[1] ?? media.sources?.[0];
  if (!src?.url) throw new Error("sem source");
  if ((media.cipher?.type ?? "").toUpperCase() !== "BF_CBC_STRIPE") throw new Error(`cifra não suportada ${media.cipher?.type}`);
  return { url: src.url, format: media.format };
}
let deezerSession;
async function deezerAccount(trackId, formats) {
  const H = { Accept: "application/json", "User-Agent": UA, "Content-Type": "application/json" };
  if (!deezerSession) {
    const res = await http(`https://www.deezer.com/ajax/gw-light.php?${qs({ method: "deezer.getUserData", input: 3, api_version: "1.0", api_token: "" })}`, { method: "POST", body: "", headers: { ...H, Cookie: `arl=${env.DEEZER_ARL}` } });
    const r = (await res.json()).results;
    if (!r?.USER?.USER_ID) throw new Error("sessão inválida (ARL expirado?)");
    const sid = res.headers.getSetCookie().map((c) => c.split(";")[0]).join("; ");
    deezerSession = { apiToken: r.checkForm, license: r.USER.OPTIONS?.license_token ?? "", cookie: `arl=${env.DEEZER_ARL}; ${sid}` };
  }
  const s = deezerSession;
  const tok = (await json(`https://www.deezer.com/ajax/gw-light.php?${qs({ method: "song.getListData", input: 3, api_version: "1.0", api_token: s.apiToken })}`, { method: "POST", body: JSON.stringify({ sng_ids: [+trackId] }), headers: { ...H, Cookie: s.cookie } })).results?.data?.[0]?.TRACK_TOKEN;
  if (!tok) throw new Error("sem TRACK_TOKEN");
  return json("https://media.deezer.com/v1/get_url", { method: "POST", headers: { ...H, Cookie: s.cookie }, body: JSON.stringify({ track_tokens: [tok], license_token: s.license, media: [{ type: "FULL", formats: formats.map((f) => ({ cipher: "BF_CBC_STRIPE", format: f })) }] }) });
}
async function deezerPlay(c) {
  const formats = { FLAC: ["FLAC", "MP3_320", "MP3_128"], MP3_320: ["MP3_320", "MP3_128"], MP3_128: ["MP3_128"] }[DEEZER_QUALITY];
  const errors = [];
  const attempts = [];
  if (env.DEEZER_ARL) attempts.push(["conta(ARL)", () => deezerAccount(c.trackId, formats)]);
  const resolvers = uniq([...(DEEZER_QUALITY === "MP3_128" ? [DEEZER_RESOLVER_128] : []), DEEZER_RESOLVER, DEEZER_RESOLVER_RENDER]);
  for (const r of resolvers) attempts.push([new URL(r).host, () => json(r, { method: "POST", timeout: 30000, headers: { Accept: "application/json", "Content-Type": "application/json", "User-Agent": UA }, body: JSON.stringify({ formats, ids: [+c.trackId], fast: false }) })]);
  for (const [name, fn] of attempts) {
    try {
      const { url, format } = deezerEnvelope(await fn(), DEEZER_QUALITY);
      let decrypt, tag = "decifrado";
      try { crypto.createDecipheriv("bf-cbc", Buffer.alloc(16), Buffer.alloc(8)); decrypt = deezerDecryptFirstBlocks(c.trackId); } catch { tag = "cifrado; rode com --openssl-legacy-provider"; }
      if (!decrypt) { const res = await http(url, { headers: { Range: "bytes=0-4095", "User-Agent": UA } }); if (!res.ok) throw new Error(`stream HTTP ${res.status}`); return `${name} ${format} → HTTP ${res.status} (${tag})`; }
      return `${name} ${format} → ${await probe(url, {}, decrypt)} (${tag} BF_CBC_STRIPE)`;
    } catch (e) { errors.push(`${name}: ${e.message}`); }
  }
  throw new Error(errors.join(" | "));
}

// ---------- YouTube Music ----------
const ytmCandidates = async (meta) => (await ytmSearch(searchTerm(meta))).slice(0, LIMIT).map((t) => ({ provider: "YouTube Music", trackId: t.id, title: t.title, artist: t.artist, album: t.album, durationMs: t.durationMs }));
async function ytmPlay(c) {
  const r = await ytResolve(c.trackId);
  const f = r.format, skipped = r.log.length ? ` [antes: ${r.log.join("; ")}]` : "";
  // o app ainda tenta WEB_REMIX primeiro e, com url em mãos, valida com HEAD antes de aceitar o cliente
  return `${r.client} itag ${f.itag} ${f.mimeType.split(";")[0]} ${Math.round(f.bitrate / 1000)}kbps → ${await probe(f.url, { "User-Agent": r.ua })}${skipped}`;
}

// ---------- Qobuz (backend "Kenny") ----------
async function qobuzSearch(meta) {
  const [title, a0, a3, album] = [meta.title.trim(), meta.artists[0] ?? "", meta.artists.slice(0, 3).join(" "), meta.album ?? ""];
  const terms = uniq([[title, a0, album], [title, a3, album], [title, a0], [title, a3], [title, album], [title]].map((p) => p.filter(Boolean).join(" ")));
  const results = new Map();
  for (const term of terms) {
    const items = await Promise.any(QOBUZ_INSTANCES.map(async (base) => {
      const root = await json(`${base}/api/get-music?${qs({ q: term, offset: 0 })}`, { headers: { Accept: "application/json", Referer: `${base}/`, "User-Agent": "Mozilla/5.0" } });
      const it = root.success ? root.data?.tracks?.items : null;
      if (!it?.length) throw new Error("vazio");
      return it;
    })).catch(() => []);
    for (const t of items) {
      const id = String(t.id);
      if (!results.has(id)) results.set(id, { provider: "Qobuz", trackId: id, title: [t.title, t.version].filter(Boolean).join(" - "), artist: t.performer?.name ?? t.album?.artist?.name ?? "", album: t.album?.title ?? null, durationMs: t.duration ? t.duration * 1000 : null, note: t.hires ? "hires" : "" });
      if (results.size >= LIMIT) return [...results.values()];
    }
  }
  return [...results.values()];
}
async function qobuzPlay(c) {
  const errors = [];
  for (const quality of [27, 7, 6, 5]) for (const base of QOBUZ_INSTANCES) {
    try {
      const root = await json(`${base}/api/download-music?${qs({ track_id: c.trackId, quality })}`, { headers: { Accept: "application/json,text/plain,*/*", Origin: base, Referer: `${base}/`, "User-Agent": UA } });
      if (!root.success) throw new Error(root.error ?? root.message ?? "rejeitado");
      const url = root.data?.url ?? root.url;
      if (!url) throw new Error("sem url");
      return `quality ${quality} → ${await probe(url)}`;
    } catch (e) { errors.push(`q${quality}: ${e.message}`); }
  }
  throw new Error(errors.slice(-2).join(" | "));
}

// ---------- execução ----------
// TIDAL e JioSaavn ficaram de fora de propósito (não serão usados no MetroMusic). Qobuz só lista com instância configurada.
const PROVIDERS = [
  ["SoundCloud", scSearch, scPlay, true],
  ["Deezer", deezerSearch, deezerPlay, true],
  ["YouTube Music", ytmCandidates, ytmPlay, true],
  ["Qobuz", qobuzSearch, qobuzPlay, false],
];
const withTimeout = (p, ms) => Promise.race([p, new Promise((_, r) => setTimeout(() => r(new Error(`timeout ${ms}ms`)), ms))]);
const fmt = (ms) => (ms ? `${Math.floor(ms / 60000)}:${String(Math.round((ms % 60000) / 1000)).padStart(2, "0")}` : "?:??");

const meta = await loadMetadata();
console.log(`Faixa: ${meta.title} — ${meta.artists.join(", ")} | álbum: ${meta.album ?? "-"} | ${fmt(meta.durationMs)} | id ${meta.id}\n`);
const groups = await Promise.all(PROVIDERS.map(async ([name, search, play, inDialog]) => {
  const t0 = Date.now();
  let cands = [], error = null;
  try { cands = await withTimeout(search(meta), 8000); } catch (e) { error = e.message; } // EXPERIMENTAL_SEARCH_TIMEOUT_MS
  const searchMs = Date.now() - t0;
  if (PLAY) await Promise.all(cands.map(async (c) => { try { c.play = await withTimeout(play(c), 60000); c.ok = true; } catch (e) { c.play = e.message; c.ok = false; } }));
  return { name, inDialog, cands, error, searchMs };
}));

let n = 0, dialog = 0, okCount = 0, total = 0;
for (const g of groups) {
  console.log(`== ${g.name}${g.inDialog ? "" : "  (sem instância no MetroFusePlus por padrão)"} — ${g.cands.length} candidato(s), busca ${g.searchMs}ms${g.error ? ` — ERRO: ${g.error}` : ""}`);
  for (const c of g.cands) {
    n++; total++;
    if (g.inDialog) dialog++;
    if (c.ok) okCount++;
    console.log(`${String(n).padStart(2)}. ${c.title} - ${c.artist}  [${c.album ?? "-"}] ${fmt(c.durationMs)}  id=${c.trackId}${c.note ? `  (${c.note})` : ""}`);
    if (PLAY) console.log(`    ${c.ok ? "✔ TOCA" : "✘ FALHA"}: ${c.play}`);
  }
  console.log();
}
console.log(`Total (SoundCloud+Deezer+YouTube Music): ${dialog}  |  com Qobuz: ${total}${PLAY ? `  |  reproduzíveis: ${okCount}/${total}` : ""}`);
if (args.includes("--json")) console.log(JSON.stringify(groups.map((g) => ({ ...g, cands: g.cands.map(({ raw, ...c }) => c) })), null, 1));
