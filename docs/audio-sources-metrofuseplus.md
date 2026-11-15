# Audio sources do MetroFusePlus — anotações para o porte

Fonte: `MetroFusePlus/app/src/main/kotlin/com/metrolist/music/{providers,deezer,qobuz,soundcloud}`.
TIDAL e JioSaavn foram descartados (não serão portados nem usados no MetroMusic).
Teste: `node --openssl-legacy-provider scripts/net/audio-sources.mjs [videoId] [--no-play] [--json]`.

## Como o diálogo "Choose audio source" monta a lista

`ProviderMatchSearch.search(metadata, perProviderLimit = 6)`: para cada provider da ordem
(SoundCloud, TIDAL, Deezer, Amazon, Apple, Instagram, YouTube Music, Qobuz, JioSaavn) roda a busca em
paralelo com timeout de 8 s, junta tudo e remove duplicados por `provider:trackId` (cache de 90 s).
Escolher um item grava um override `mediaId → {provider, trackId}`; na reprodução o id vira
`tidal:track:<id>`, `deezer:track:<id>`, `qobuz:track:<id>` (SoundCloud = permalink, YouTube = videoId) e
vai direto ao `resolve` do provider, sem refazer o match.

Billie Jean (`Kr4EQDVETuA`) = 6 SoundCloud + 1 TIDAL + 6 Deezer + 6 YouTube Music = **19**.
Qobuz dá 0 sem instância configurada; JioSaavn, Amazon, Apple e Instagram não geram candidatos no diálogo.

| Provider | Busca dos candidatos | Termo |
|---|---|---|
| SoundCloud | `api-v2.soundcloud.com/search/tracks?q&limit=6&client_id` (só `kind=track` e `streamable`) | título + 1º artista + álbum |
| Deezer | `api.deezer.com/track/isrc:<isrc>` (se houver) + `api.deezer.com/search/track?q&limit=12`, até 6 ids únicos | variações de título × artista/álbum |
| YouTube Music | busca innertube com filtro de músicas, 6 primeiros | título + 1º artista + álbum |
| Qobuz | `<instância>/api/get-music?q&offset=0` (corrida entre instâncias) | isrc, depois título/artista/álbum |

## Como cada provider tenta tocar

- **SoundCloud** — `client_id` raspado da home/bundles JS (cache 12 h). Pega `media.transcodings`
  (ou `/tracks/<id>`), escolhe por qualidade (padrão AAC 160: bitrate/mime alvo → aac → hls → maior
  bitrate), chama a URL do transcoding com `client_id` + `track_authorization` e toca o `url` devolvido
  (HLS ou progressivo). Headers `Referer/Origin: soundcloud.com`.
- **Deezer** — 1º conta (cookie `arl`): `gw-light.php?method=deezer.getUserData` → `checkForm` +
  `USER.OPTIONS.license_token`; `song.getListData {sng_ids}` → `TRACK_TOKEN`; `POST media.deezer.com/v1/get_url`
  com `{track_tokens, license_token, media:[{type:FULL, formats:[{cipher:BF_CBC_STRIPE, format}]}]}`.
  2º resolver: `POST <resolver>/get_url {formats, ids:[id], fast:false}` (mesmo envelope de resposta),
  primeiro o lote e depois uma qualidade por vez (`FLAC → MP3_320 → MP3_128`). Resolvers no código:
  `api-number-2-poopo.onrender.com` (só MP3_128), `yesitworkssomehow-funny-deeza-api-and-yeah.hf.space`,
  `dzmedia-metrofuse.onrender.com`. Usa `sources[1]` (ou `[0]`) e decifra em tempo real: blocos de 2048
  bytes, só os de índice múltiplo de 3 (e completos) em Blowfish-CBC, IV `00..07`,
  chave `md5hex(id)[i] ^ md5hex(id)[i+16] ^ "g4el58wc0zvf9na1"[i]`.
- **Qobuz** — sem login; backend "Kenny" em instâncias configuradas pelo usuário (padrão vazio).
  `GET <inst>/api/download-music?track_id&quality=27→7→6→5` → `data.url` direta (FLAC ou MP3);
  recusa prévias (duração <45 s ou bitrate baixo demais).
- **YouTube Music** — pipeline normal do innertube (o MetroMusic já tem o próprio).
