// Teste de verdade do Ouvir junto: o código real do app (src/lib/lt/session.ts + player) contra o servidor público,
// com um par "de mentira" do outro lado (cliente cru do protocolo) e um motor de áudio falso.
// Precisa de internet. Uso: node scripts/net/lt.session.net.mjs [wss://…]
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import { makeCodec } from "../../src/lib/lt/protocol.ts";
import { LTClient } from "../../src/lib/lt/client.ts";
import { installApp, loadApp, sleep, until } from "../tests/harness.mjs";

const url = process.argv[2] ?? "wss://metroserverx.meowery.eu/ws";
const codec = makeCodec(readFileSync("src/lib/lt/listentogether.proto", "utf8"));
const T1 = { id: "dQw4w9WgXcQ", title: "Faixa Um", artist: "Artista", album: "", duration: 200000, thumbnail: "" };
const T2 = { id: "9bZkp7q19f0", title: "Faixa Dois", artist: "Artista", album: "", duration: 200000, thumbnail: "" };
const ext = (t) => ({ provider: "youtube", id: t.id, title: t.title, artists: [t.artist], durationMs: t.duration });
const step = (m) => console.log("✔", m);

/** Cliente cru: guarda tudo o que chega e deixa esperar por uma mensagem. */
function raw(name) {
  const inbox = []; const c = new LTClient({ codec, url, onMessage: (type, payload) => inbox.push({ type, payload, at: Date.now() }) });
  c.inbox = inbox;
  c.wait = async (type, pred = () => true, ms = 12000, from = 0) => {
    const t0 = Date.now();
    for (;;) {
      const m = inbox.slice(from).find((x) => x.type === type && pred(x.payload));
      if (m) return m.payload;
      if (Date.now() - t0 > ms) throw new Error(`[${name}] esperei ${type} por ${ms} ms; chegou: ${inbox.slice(from).map((m) => m.type + (m.payload.action ? ":" + m.payload.action : "")).join(",") || "nada"}`);
      await sleep(50);
    }
  };
  c.act = (action, extra = {}) => c.send("playback_action", { action, serverTime: Date.now(), ...extra });
  return c;
}
const near = (a, b, tol, what) => assert.ok(Math.abs(a - b) <= tol, `${what}: ${a.toFixed(2)} s, esperado ${b.toFixed(2)} s (±${tol})`);

async function boot(opts) {
  const env = installApp({}, opts);
  const app = await loadApp(["/src/lib/lt/session.ts", "/src/store/player.ts", "/src/lib/catalog.ts", "/src/store/settings.ts"]);
  const { useLT } = app.mods["/src/lib/lt/session.ts"];
  const { usePlayer, initBackend } = app.mods["/src/store/player.ts"];
  const { registerTrack } = app.mods["/src/lib/catalog.ts"];
  app.mods["/src/store/settings.ts"].setSetting("ltServer", "custom");
  app.mods["/src/store/settings.ts"].setSetting("ltCustomServer", url);
  await initBackend();
  return { ...env, app, useLT, usePlayer, registerTrack, setSetting: app.mods["/src/store/settings.ts"].setSetting };
}

// ───────────────────────── 1) o app como CONVIDADO
async function asGuest(loadMs, label) {
  console.log(`\n== App como convidado (${label})`);
  const { useLT, usePlayer: P, fake, app } = await boot({ loadMs });
  const H = raw("anfitrião"); await H.connect();
  try {
    H.send("create_room", { username: "AnfitriaoCru" });
    const { roomCode } = await H.wait("room_created");
    H.act("change_track", { trackId: T1.id, trackInfo: T1, position: 0 });
    H.send("buffer_ready", { trackId: T1.id }); await H.wait("buffer_complete");
    H.act("play", { trackId: T1.id, position: 0 });
    const hostStart = Date.now(); const hostAt = () => (Date.now() - hostStart) / 1000;
    await sleep(3000); // a música já anda quando o app entra
    await useLT.getState().join("MmConvidado", roomCode);
    const req = await H.wait("join_request"); H.send("approve_join", { userId: req.userId });
    await until("convidado entrou", () => useLT.getState().room === roomCode, 8000);
    step(`entrou na sala ${roomCode}`);

    await until("faixa da sala carregada", () => P.getState().queue[P.getState().index] === `youtube:${T1.id}`, 8000);
    await until("tocando", () => P.getState().playing && !P.getState().buffering && fake.playing, 15000);
    near(P.getState().position, hostAt(), 2.2, "entrou no meio da música: posição");
    assert.equal(fake.plays.filter((p) => p.key === `youtube:${T1.id}`).length, 1, "não pode recarregar a faixa à toa");
    step("entrou no meio da música e já toca perto do anfitrião");

    H.act("pause", { trackId: T1.id, position: 20000 });
    await until("pausou", () => !P.getState().playing && !fake.playing, 6000);
    near(P.getState().position, 20, 1.5, "pausa: posição"); step("pausa do anfitrião chegou, na posição dele");

    H.act("play", { trackId: T1.id, position: 20000 });
    await until("voltou a tocar", () => P.getState().playing && fake.playing, 6000);
    near(P.getState().position, 20, 2, "play: posição"); step("play do anfitrião chegou");

    H.act("seek", { trackId: T1.id, position: 100000 });
    await until("foi para 100 s", () => P.getState().position > 99 && P.getState().position < 104, 6000); step("seek do anfitrião chegou");

    H.act("change_track", { trackId: T2.id, trackInfo: T2, position: 0 });
    await until("trocou de faixa", () => P.getState().queue[P.getState().index] === `youtube:${T2.id}`, 6000);
    await until("nova faixa pronta", () => fake.ready && !P.getState().buffering, 8000);
    assert.equal(P.getState().playing, false, "espera o anfitrião mandar tocar");
    near(P.getState().position, 0, 0.8, "nova faixa começa parada no 0");
    H.send("buffer_ready", { trackId: T2.id }); await H.wait("buffer_complete", (p) => p.trackId === T2.id);
    H.act("play", { trackId: T2.id, position: 0 });
    await until("toca a nova faixa", () => P.getState().playing && fake.playing, 8000);
    near(P.getState().position, 0.5, 2, "nova faixa: posição"); step("troca de faixa: espera o anfitrião e toca junto");

    // a faixa demorou a carregar enquanto o anfitrião andava: ao ficar pronta, pula para onde ele está
    const before = fake.plays.length;
    H.act("change_track", { trackId: T1.id, trackInfo: T1, position: 0 });
    H.act("play", { trackId: T1.id, position: 0 });
    const t0 = Date.now();
    await until("recarregou", () => fake.plays.length > before && P.getState().playing && !P.getState().buffering && fake.playing, 10000);
    near(P.getState().position, (Date.now() - t0) / 1000, 2.2, "depois de carregar, alinhado com o anfitrião"); step("alinha depois de carregar");

    useLT.getState().leave();
    assert.equal(useLT.getState().room, null); step("saiu da sala");
  } finally { H.close(); useLT.getState().disconnect(); await app.close(); }
}

// ───────────────────────── 2) o app como ANFITRIÃO
async function asHost() {
  console.log("\n== App como anfitrião");
  const { useLT, usePlayer: P, fake, app, registerTrack, setSetting } = await boot({ loadMs: 250 });
  const G1 = raw("convidado 1"), G2 = raw("convidado 2"); await Promise.all([G1.connect(), G2.connect()]);
  try {
    setSetting("ltAutoJoin", true);
    const t1 = registerTrack(ext(T1)), t2 = registerTrack(ext(T2));
    P.getState().play([t1.id, t2.id], 0, "teste");
    await until("tocando", () => P.getState().playing && fake.playing && !P.getState().buffering, 5000);
    await sleep(2500);
    await useLT.getState().create("MmAnfitriao");
    await until("sala criada", () => !!useLT.getState().room, 8000);
    const code = useLT.getState().room; step(`sala criada: ${code}`);

    G1.send("join_room", { roomCode: code, username: "Convidado1" });
    const joined = await G1.wait("join_approved");
    assert.equal(joined.state.currentTrack?.id, T1.id); step("convidado recebe a faixa atual no estado da sala");
    // o anfitrião carregou, avisou o servidor e mandou tocar
    await until("sala tocando", async () => true, 1);
    G1.send("request_sync");
    const st = await G1.wait("sync_state", (s) => s.isPlaying, 9000);
    near(st.position / 1000, P.getState().position, 3, "posição da sala"); step("sala marcada como tocando, na posição do anfitrião (sem esperar 8 s)");

    // um segundo convidado entra: ninguém pode ter a faixa reiniciada
    const mark = G1.inbox.length;
    G2.send("join_room", { roomCode: code, username: "Convidado2" });
    const j2 = await G2.wait("join_approved");
    assert.equal(j2.state.currentTrack?.id, T1.id);
    await sleep(2500);
    const restarts = G1.inbox.slice(mark).filter((m) => m.type === "sync_playback" && m.payload.action === "change_track");
    assert.equal(restarts.length, 0, "a entrada de outro convidado reiniciou a faixa para quem já estava na sala");
    near(P.getState().position, fake.pos, 0.6, "o anfitrião segue tocando"); assert.ok(P.getState().position > 4, "faixa do anfitrião não reiniciou");
    step("entrada de outro convidado não reinicia a música");

    let m = G1.inbox.length;
    P.getState().toggle();
    const pa = await G1.wait("sync_playback", (p) => p.action === "pause", 6000, m);
    near(pa.position / 1000, P.getState().position, 1, "pausa: posição enviada"); step("pausa do anfitrião chega aos convidados");
    m = G1.inbox.length; P.getState().toggle();
    await G1.wait("sync_playback", (p) => p.action === "play", 6000, m); step("play do anfitrião chega aos convidados");
    m = G1.inbox.length; P.getState().seek(150);
    const sk = await G1.wait("sync_playback", (p) => p.action === "seek", 6000, m);
    near(sk.position / 1000, 150, 1, "seek enviado"); step("seek do anfitrião chega aos convidados");

    m = G1.inbox.length; const t0 = Date.now();
    P.getState().next();
    const ct = await G1.wait("sync_playback", (p) => p.action === "change_track", 6000, m);
    assert.equal(ct.trackInfo.id, T2.id);
    G1.send("buffer_ready", { trackId: T2.id }); G2.send("buffer_ready", { trackId: T2.id });
    const pl = await G1.wait("sync_playback", (p) => p.action === "play" && p.trackId === T2.id, 7000, m);
    assert.ok(Date.now() - t0 < 6500, "o anfitrião esperou o tempo limite em vez do aviso de carregamento");
    near(pl.position / 1000, 0.5, 3, "play da nova faixa"); step(`troca de faixa: anfitrião manda tocar em ${((Date.now() - t0) / 1000).toFixed(1)} s`);

    useLT.getState().leave(); step("anfitrião saiu");
  } finally { G1.close(); G2.close(); useLT.getState().disconnect(); await app.close(); }
}

try {
  await asGuest(250, "faixa carrega rápido");
  await asGuest(2500, "faixa carrega devagar (2,5 s)");
  await asHost();
  console.log("\nTUDO OK contra", url);
} catch (e) { console.error("FALHOU:", e.stack ?? e.message); process.exitCode = 1; }
finally { setTimeout(() => process.exit(process.exitCode ?? 0), 300); }
