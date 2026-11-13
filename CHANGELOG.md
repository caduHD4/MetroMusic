# Changelog

**English** · [Português (Brasil)](CHANGELOG.pt-BR.md)

Each version published on GitHub uses the text of its section as the release notes (it is what appears in the in-app update notice).

## 0.1.5

Audio now follows the system output device.

### Audio
- **Switching the Windows output device now works while the app is open.** MetroMusic follows the system default (headphones, speakers, HDMI, Bluetooth…) and the sound moves with it. Before, it stayed on the device that was selected when the app started. When the new device uses a different sample rate, the current track reloads from the same spot, and stays paused if it was paused.
- On Linux nothing changes: PipeWire/PulseAudio already move the sound (checked by switching the default sink while the audio engine was playing).

## 0.1.4

Fixes for playback when the app opens and for Listen together.

### Playback
- **Music no longer starts "paused and silent" after opening the app.** The restored queue is now really loaded when you press play, and resumes from where you stopped. The same applies to a track that ended while autoplay was off.
- Pausing, resuming or seeking while a track is still loading now takes effect. Before, the request was lost and the music started by itself, from the beginning.

### Listen together
- Joining in the middle of a song now starts **at the host's position** (it used to play from 0:00).
- When someone joins the room, the song **no longer restarts** for the people already in it.
- The host reports when the track has loaded, so the room starts playing right away instead of waiting 8 seconds on every track change.
- The position follows the host's clock: if a track takes long to load, it jumps to where the host is, and a periodic check fixes drift over time.
- Tested end to end against the real server (`pnpm test:net`), with the app as both host and guest.

### Documentation
- README and changelog are now in English by default, with the Portuguese version alongside.

## 0.1.3

New background option.

### Appearance
- New **"Ada"** background option for the app and for Now playing (contributed by Helon21).

## 0.1.2

The Linux AppImage now uses the graphics card.

### Linux
- **The AppImage is built on Ubuntu 24.04**, bundling WebKit 2.52 (before, it shipped an older WebKit that, on some systems such as CachyOS with an NVIDIA card, drew everything on the CPU and made the interface slow). With the new one, WebKit initializes the GPU.
- *Settings › Appearance › Visual performance* now shows **which graphics card the app is using** (or "software rendering") and whether light mode is on, so you can check at a glance.
- Package requirement: systems with glibc 2.39 or newer (Ubuntu 24.04+, Debian 13+, Fedora 40+, Arch and derivatives). On older distributions, build from source.

## 0.1.1

Performance fix: the app now runs smoothly on any PC, including ones without a graphics card.

### Performance
- **Automatic light mode:** if the PC has no GPU (software rendering) or the app notices slowness, it turns off the giant blurs, large shadows, decorative animations and background videos by itself. People with a GPU keep all the effects, nothing changes.
- New option in *Settings › Appearance › Visual performance*: Automatic (default), Full or Light.
- In light mode, the blurred Now playing background becomes a gradient with the cover's color, and lyrics animate at 30 frames per second.
- On any PC: animations pause when the window is hidden or unfocused, and the lyrics blur only applies to the lines near the current one.
- Tested with software rendering: the CPU cost of Home dropped from 15% to 3% of one core, and Now playing with a moving blur from 117% to 16%.

### Linux
- The AppImage ships its own WebKit and, on some systems (for example with an NVIDIA card), it does not use the GPU. Light mode covers that case. If you prefer the system WebKit, use the `.deb` package or build from source.

## 0.1.0

First public release of MetroMusic.

### Listening
- YouTube Music and SoundCloud play directly in the app; per-track source choice showing only valid results.
- Configurable audio quality, 10-band equalizer, normalization, crossfade and gapless playback between tracks.
- Sleep timer, alarms and automatic sleep.

### Lyrics
- Word-by-word synced lyrics (BetterLyrics, LrcLib, KuGou, Spotify), with romanization (Japanese, Korean, Chinese, Cyrillic, Hindi and Punjabi) and AI translation.

### Accounts and services
- Sign in to YouTube Music, Spotify, SoundCloud and Last.fm; likes and playlists stay in sync.
- **Discord Rich Presence** with no setup: shows the song, the time and the pause on your profile.
- **Listen together** with rooms compatible with MetroFuse and Metrolist.
- Spotify Canvas over the cover art (with a connected Spotify account).

### Library
- Downloads with title, artist, album, cover and lyrics written into the file.
- Local library: scans your folders, reads the tags and follows changes.
- Import playlists from M3U, CSV and YouTube Music and Spotify links.
- Backup and restore.

### Appearance
- Themes, palettes, "Galaxy" and blurred backgrounds, Portuguese and English languages.

### Updates
- The app tells you when there is a new version, shows what changed and updates with one click, with no installer.
