import test from "node:test";
import assert from "node:assert/strict";
import { installApp, loadApp, sleep, until } from "./harness.mjs";

const ext = (id, over = {}) => ({ provider: "youtube", id, title: `Faixa ${id}`, artists: ["Artista"], durationMs: 200000, ...over });

// Estado salvo de uma sessão anterior: fila com 3 faixas reais, na segunda, aos 73 s, parada.
const saved = {
  "metromusic-ext-tracks-v1": JSON.stringify(["aaa", "bbb", "ccc"].map((i) => ext(i))),
  "metromusic-player-v1": JSON.stringify({ state: { queue: ["youtube:aaa", "youtube:bbb", "youtube:ccc"], index: 1, position: 73, volume: 0.6, muted: false, repeat: "off", shuffle: false, queueTitle: "Salva", speed: 1, pitch: 0 }, version: 0 }),
};

async function boot(storage) {
  const { tauri, fake } = installApp(storage);
  const app = await loadApp(["/src/store/player.ts", "/src/lib/catalog.ts", "/src/store/settings.ts"]);
  const { usePlayer, initBackend } = app.mods["/src/store/player.ts"];
  app.mods["/src/lib/catalog.ts"].loadExternalTracks();
  await initBackend();
  return { usePlayer, tauri, fake, app };
}

test("abrir o app e apertar play: carrega a faixa restaurada de onde parou e toca de verdade", async () => {
  const { usePlayer, fake, tauri, app } = await boot(saved);
  try {
    const P = usePlayer;
    assert.equal(P.getState().playing, false);
    assert.equal(fake.loaded, null, "o motor começa vazio");
    P.getState().toggle();
    await until("motor carregou a faixa", () => fake.loaded === "youtube:bbb");
    assert.equal(fake.start, 73, "retoma da posição salva");
    await until("tocando", () => fake.playing);
    await until("interface em reprodução", () => P.getState().playing && !P.getState().buffering);
    assert.equal(tauri.named("audio_resume").length, 0, "não pode depender de resume num motor vazio");
    // pausar e retomar agora usam pause/resume
    P.getState().toggle(); await until("pausou", () => !P.getState().playing && fake.playing === false);
    P.getState().toggle(); await until("retomou", () => P.getState().playing && fake.playing);
    assert.equal(tauri.named("audio_play").length, 1);
  } finally { await sleep(30); await app.close(); }
});

test("faixa que terminou com o autoplay desligado: play carrega de novo do começo (o motor ficou no fim)", async () => {
  const { usePlayer, fake, tauri, app } = await boot(saved);
  try {
    const P = usePlayer;
    app.mods["/src/store/settings.ts"].setSetting("autoplay", false);
    P.getState().play(["youtube:aaa", "youtube:bbb"], 0, "x");
    await until("tocando", () => P.getState().playing && !P.getState().buffering && fake.loaded === "youtube:aaa");
    tauri.emit("audio:ended", { token: fake.token, key: "youtube:aaa" });
    await until("parou no fim", () => !P.getState().playing);
    assert.equal(P.getState().index, 0, "sem autoplay não avança");
    P.getState().toggle();
    await until("carregou outra vez", () => tauri.named("audio_play").length === 2 && P.getState().playing);
    assert.equal(fake.start, 0);
    assert.equal(tauri.named("audio_resume").length, 0);
  } finally { await sleep(30); await app.close(); }
});

test("sem fila persistente salva (primeiro uso, faixas de demonstração): play não quebra", async () => {
  const { usePlayer, app } = await boot({});
  try { usePlayer.getState().toggle(); await sleep(10); assert.ok(true); } finally { await sleep(30); await app.close(); }
});

test("pausar enquanto a faixa ainda carrega: ela carrega pausada e não toca sozinha", async () => {
  const { usePlayer, fake, app } = await boot(saved);
  try {
    const P = usePlayer;
    P.getState().play(["youtube:aaa", "youtube:bbb"], 0, "x");
    P.getState().toggle(); // pausa antes de carregar
    assert.equal(P.getState().playing, false);
    await until("carregou", () => fake.ready);
    assert.equal(fake.playing, false, "o motor não pode começar a tocar depois de pausado");
    await until("interface sem carregando", () => !P.getState().buffering);
    assert.equal(P.getState().playing, false);
    P.getState().toggle(); // e retomar depois de pausado toca
    await until("tocando", () => fake.playing && P.getState().playing);
  } finally { await sleep(30); await app.close(); }
});

test("faixa carregada em pausa (Ouvir junto) fica pausada, sem 'carregando', e retoma com play", async () => {
  const { usePlayer, fake, tauri, app } = await boot(saved);
  try {
    const P = usePlayer;
    P.getState().play(["youtube:aaa"], 0, "sala", { paused: true });
    assert.equal(P.getState().playing, false);
    await until("carregou", () => fake.ready && !P.getState().buffering);
    assert.equal(fake.playing, false);
    assert.equal(tauri.named("audio_play")[0].args.autoplay, false);
    P.getState().toggle();
    await until("tocando", () => fake.playing && P.getState().playing);
    assert.equal(tauri.named("audio_play").length, 1, "retomar não recarrega");
  } finally { await sleep(30); await app.close(); }
});
