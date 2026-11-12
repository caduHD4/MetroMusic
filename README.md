<div align="center">

**English** · [Português (Brasil)](README.pt-BR.md)

<img src="src-tauri/icons/128x128@2x.png" alt="MetroMusic icon" width="160" />

# MetroMusic

### Desktop music player: YouTube Music, SoundCloud and Spotify in one place, with word-by-word lyrics, Discord and Listen together.

<br/>

[![Latest version](https://img.shields.io/github/v/release/caduHD4/MetroMusic?style=for-the-badge&labelColor=0d1117&color=a78bfa)](https://github.com/caduHD4/MetroMusic/releases/latest)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue?style=for-the-badge&labelColor=0d1117)](LICENSE)
[![Downloads](https://img.shields.io/github/downloads/caduHD4/MetroMusic/total?style=for-the-badge&labelColor=0d1117)](https://github.com/caduHD4/MetroMusic/releases)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%C2%B7%20Windows-4c1d95?style=for-the-badge&labelColor=0d1117)](#install)
[![CI](https://img.shields.io/github/actions/workflow/status/caduHD4/MetroMusic/ci.yml?branch=main&style=for-the-badge&labelColor=0d1117&label=tests)](https://github.com/caduHD4/MetroMusic/actions/workflows/ci.yml)

<br/>

[**Screenshots**](#screenshots) · [**Features**](#features) · [**Install**](#install) · [**Updates**](#automatic-updates) · [**Services**](#accounts-and-services) · [**Build**](#build) · [**Credits**](#credits)

</div>

> [!WARNING]
> MetroMusic connects to third-party services chosen by you. Availability, quality, catalog, login and regional access may change at any time and may require your own account. It is an independent, unofficial project, with no affiliation with YouTube, Google, Spotify, Discord, SoundCloud or any other service mentioned.

---

## Screenshots

<div align="center">

<img src="docs/screenshots/01-inicio.png" alt="Home" width="100%" />

<sub>**Home:** the content of your YouTube Music account (or another source), with mood shortcuts.</sub>

<br/><br/>

<img src="docs/screenshots/02-tocando-agora.png" alt="Now playing, with synced lyrics" width="100%" />

<sub>**Now playing:** word-by-word lyrics, a "Galaxy" background that follows the cover art, and the lyric line under the cover.</sub>

<br/><br/>

<img src="docs/screenshots/03-biblioteca.png" alt="Library" width="49%" />
<img src="docs/screenshots/04-explorar.png" alt="Explore" width="49%" />

<img src="docs/screenshots/06-equalizador.png" alt="Equalizer" width="49%" />
<img src="docs/screenshots/05-configuracoes.png" alt="Settings" width="49%" />

<img src="docs/screenshots/08-boas-vindas.png" alt="Welcome: connect your services" width="49%" />
<img src="docs/screenshots/07-atualizacao.png" alt="Update notice with what changed" width="49%" />

</div>

---

## What is MetroMusic?

A desktop music player built with **Tauri (Rust)** and **React**, inspired by [MetroFuse](https://github.com/956tris/MetroFuse) and [Metrolist](https://github.com/MetrolistGroup/Metrolist). It brings several audio and catalog sources into a single interface, plays with its own engine (equalizer, normalization, crossfade and gapless playback) and integrates with Discord, Last.fm and Spotify.

Everything that needs an account is optional: you can use it with just search and playback.

---

## Features

| Playback | Discovery |
| --- | --- |
| Own audio engine in Rust (10-band equalizer, normalization, crossfade, gapless) | Home, Explore and artist/album pages from YouTube Music |
| Per-track audio source choice, showing only valid results | Search for songs, albums, artists and playlists |
| Song and cover cache on disk, with a size limit | Import playlists from M3U, CSV and YouTube Music and Spotify links |
| Downloads with title, artist, album, cover and lyrics written into the file | Local library: scans your folders and follows changes |
| Sleep timer, alarms and automatic sleep | Listening history and statistics |

| Lyrics | Integrations |
| --- | --- |
| Word-by-word synced lyrics (BetterLyrics, LrcLib, KuGou and Spotify) | **Discord Rich Presence** with no setup, with time and pause |
| Romanization: Japanese, Korean, Chinese, Cyrillic, Hindi and Punjabi | **Listen together**, compatible with MetroFuse and Metrolist rooms |
| AI translation (OpenRouter, OpenAI, Claude, Gemini, DeepL and others) | YouTube Music, Spotify, SoundCloud and Last.fm: likes and playlists in sync |
| `.lrc` next to the file or lyrics saved in the tag, for the local library | Spotify Canvas over the cover (with a Spotify account) |

| Appearance | System |
| --- | --- |
| Themes, palettes and "Galaxy" and blurred backgrounds | Automatic updates through GitHub, no installer |
| Automatic light mode for PCs without a graphics card | Tray, global shortcuts, system media controls (MPRIS), launch at login |
| Languages: Portuguese and English | HTTP and SOCKS5 proxy, settings in a file and secrets kept apart |
| Backup and restore of your data | |

---

## Install

Download the latest version from the [releases page](https://github.com/caduHD4/MetroMusic/releases/latest).

| System | File | Note |
| --- | --- | --- |
| **Linux** | `MetroMusic_*_amd64.AppImage` | Recommended: it is the one that **updates itself**. Give it permission (`chmod +x`) and open it. |
| Linux (Debian/Ubuntu) | `MetroMusic_*_amd64.deb` | Installs system-wide, but automatic updates need administrator permission, so it does not update itself. |
| **Windows** | `MetroMusic_*_x64-setup.exe` | The first run installs normally; **later updates are silent**. |

There is no macOS package yet.

### Windows warning ("Unknown publisher")

When you open the installer, Windows Defender SmartScreen may show **"Windows protected your PC"** and **"Unknown publisher"**. This happens because the installer does **not have a code signature yet** (a paid certificate that new, free projects usually do not have), not because something is wrong with the file. To continue:

1. Click **More info**.
2. Click **Run anyway**.

The code is open source and every package in the release is signed with the project's key (that signature is what the updater checks). If you prefer, build from source (see [Build](#build)).

### Linux dependencies

The AppImage is built on Ubuntu 24.04, so it needs **glibc 2.39 or newer** (Ubuntu 24.04+, Debian 13+, Fedora 40+, Arch and derivatives such as CachyOS). On older distributions, build from source. The AppImage already bundles almost everything. For audio and the tray, the system needs `alsa` (or PipeWire with `pipewire-alsa`) and `libayatana-appindicator`.

---

## Automatic updates

MetroMusic updates itself like Discord does: **with no install wizard**.

1. On launch (and every 6 hours), the app looks for a new version on GitHub.
2. If there is one, a **small notice with what changed** appears, with an **Update** button.
3. When you click it, the app downloads in the background, swaps the files and **restarts by itself**.

Details:

- The **`main` branch is the stable channel**: updates always come from the latest published release.
- Every package is **signed**; the app only installs what matches the public key embedded in it.
- You can **skip a version** or turn the notice off in *Settings › Updates*, and check manually whenever you want.

---

## Accounts and services

The first time you open the app, a screen shows how to connect the services. Everything is optional and can be done later in *Settings › Integrations*.

| Service | What it does | Login needed? |
| --- | --- | --- |
| **YouTube Music** | Main audio source. Home, playlists and likes from your account | Optional |
| **Spotify** | Your library and playlists, lyrics and Canvas | Optional |
| **Discord** | Shows on your profile what you are listening to, with time and pause | **No**: just leave Discord open |
| SoundCloud | Audio source and likes | Optional |
| Last.fm | Scrobbling and "now playing" | Optional |

Sessions and keys stay **only on your computer**, in a separate file (permission `0600`).

**Unavailable for now:** TIDAL, Qobuz and Deezer (protected audio).

---

## Build

Prerequisites: [Node.js](https://nodejs.org) 22+, [pnpm](https://pnpm.io), [Rust](https://rustup.rs) and the [Tauri dependencies](https://v2.tauri.app/start/prerequisites/) for your system. On Linux also: `libasound2-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`.

```bash
pnpm install
pnpm tauri dev      # opens the app in development mode
pnpm tauri build    # builds the packages (needs the signing keys for the updater)
```

### Tests

```bash
pnpm exec tsc --noEmit                 # types
node --test scripts/tests/*.test.mjs   # interface tests (logic and player)
cd src-tauri && cargo test --lib       # backend tests
pnpm test:net                          # Listen together against the real server (needs internet)
```

There are also network tests (`cargo test net_ -- --ignored`) that talk to the real services.

### How to release a version

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Describe the changes in a new `## x.y.z` section (in both files) of [`CHANGELOG.md`](CHANGELOG.md) and [`CHANGELOG.pt-BR.md`](CHANGELOG.pt-BR.md): it is the text shown in the update notice.
3. Merge into `main` and create the tag: `git tag vx.y.z && git push origin vx.y.z`.
4. GitHub Actions builds, signs and publishes the release with the `latest.json` the app uses.

The signing keys (`TAURI_SIGNING_PRIVATE_KEY` and its password) live in the repository *secrets*. **Keep a copy of the private key**: without it, old versions will no longer accept updates.

---

## Documentation

- [`docs/BACKEND.md`](docs/BACKEND.md): how the Rust backend is organized (in Portuguese).
- [`docs/CONFIGURACOES-AUDITORIA.md`](docs/CONFIGURACOES-AUDITORIA.md): what every setting does, how it was validated and what is still "Coming soon" (in Portuguese).

---

## Credits

- [MetroFuse](https://github.com/956tris/MetroFuse), [MetroFuse+](https://github.com/caduHD4/MetroFusePlus) and [Metrolist](https://github.com/MetrolistGroup/Metrolist): the base of ideas and integrations (lyrics, Listen together, providers).
- [InnerTune](https://github.com/z-huang/InnerTune) and [OuterTune](https://github.com/DD3Boh/OuterTune), where Metrolist comes from.
- Lyrics: [BetterLyrics](https://github.com/boidushya/better-lyrics), [LrcLib](https://lrclib.net), KuGou and Spotify.
- Built with [Tauri](https://tauri.app), [React](https://react.dev), [Symphonia](https://github.com/pdeljanov/Symphonia), [lofty](https://github.com/Serial-ATA/lofty-rs) and [protobuf.js](https://github.com/protobufjs/protobuf.js).

## License

[GPL-3.0](LICENSE), the same license as MetroFuse and Metrolist.
