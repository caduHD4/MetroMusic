//! Provedores de letra adicionais, portados do MetroFuse (`app/.../lyrics/*` e módulos `paxsenix`, `kugou`...):
//! Apple Music (Paxsenix), Musixmatch, LyricsPlus, Binimum, QQ Music, YouTube Music e Deezer.
//!
//! Cada função devolve as linhas já com o tempo por palavra quando a fonte tem; `lyrics::get` cuida da ordem,
//! do tempo limite por provedor e do cache.

use crate::http::CLIENT;
use crate::lyrics::{clean_artist, clean_title, drop_credits, parse_lrc, parse_ttml, plain_lines, Line, Lyrics, Word};
use crate::model::TrackInfo;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use serde_json::Value;
use std::time::Duration;

const PAX: &str = "https://lyrics.paxsenix.org";
const PAX_UA: &str = "MetroMusic/0.1";
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

fn num(v: &Value, k: &str) -> Option<f64> { v.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))) }
fn txt<'a>(v: &'a Value, k: &str) -> Option<&'a str> { v.get(k).and_then(|x| x.as_str()) }
fn secs(t: &TrackInfo) -> f64 { t.duration_ms.unwrap_or(0) as f64 / 1000.0 }

fn mk_line(words: Vec<Word>, t: f64, end: Option<f64>, bg: bool, agent: Option<String>) -> Option<Line> {
    let words: Vec<Word> = words.into_iter().filter(|w| !w.w.trim().is_empty()).collect();
    if words.is_empty() { return None; }
    let text = words.iter().map(|w| w.w.as_str()).collect::<Vec<_>>().join(" ");
    let end = end.or_else(|| words.last().map(|w| w.end)).unwrap_or(t + 3.0).max(t + 0.2);
    Some(Line { t, end, text, words, bg, agent })
}

/// Junta sílabas em palavras: uma sílaba só abre palavra nova se a anterior terminou em espaço ou ela mesma começa com um.
fn join_syllables(parts: &[(String, f64, f64)]) -> Vec<Word> {
    let mut out: Vec<Word> = vec![];
    let mut glue = false; // a próxima sílaba continua a palavra atual
    for (text, t, end) in parts {
        let starts_space = text.starts_with(char::is_whitespace);
        let word = text.trim();
        if word.is_empty() { glue = false; continue; }
        if glue && !starts_space {
            if let Some(last) = out.last_mut() { last.w.push_str(word); last.end = *end; }
        } else {
            out.push(Word { t: *t, end: *end, w: word.to_string() });
        }
        glue = !text.ends_with(char::is_whitespace);
    }
    out
}

fn lyrics(provider: &str, synced: bool, lines: Vec<Line>) -> Option<Lyrics> {
    (!lines.is_empty()).then(|| Lyrics { synced, provider: provider.into(), lines })
}

// ───────── Apple Music (busca no catálogo da Apple + letra pelo Paxsenix)

static APPLE_TOKEN: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

async fn apple_token(fresh: bool) -> Option<String> {
    if !fresh { if let Some(t) = APPLE_TOKEN.lock().clone() { return Some(t); } }
    let v: Value = CLIENT.get("https://yesitworkssomehow-funny-deeza-api-and-yeah.hf.space/apple/token").header("User-Agent", PAX_UA)
        .timeout(Duration::from_secs(10)).send().await.ok()?.json().await.ok()?;
    let t = txt(&v, "token")?.to_string();
    (!t.is_empty()).then(|| { *APPLE_TOKEN.lock() = Some(t.clone()); t })
}

struct AppleHit { id: String, name: String, artist: String, dur_ms: Option<f64> }

async fn apple_search(query: &str) -> Vec<AppleHit> {
    for fresh in [false, true] {
        let Some(token) = apple_token(fresh).await else { return vec![] };
        let Ok(resp) = CLIENT.get("https://amp-api.music.apple.com/v1/catalog/us/search")
            .query(&[("term", query), ("types", "songs"), ("limit", "25"), ("l", "en-US"), ("platform", "web"), ("format[resources]", "map"), ("include[songs]", "artists"), ("extend", "artistUrl")])
            .header("Authorization", format!("Bearer {token}")).header("Origin", "https://music.apple.com").header("Referer", "https://music.apple.com/")
            .header("User-Agent", BROWSER).header("Accept", "application/json").header("x-apple-renewal", "true").timeout(Duration::from_secs(10)).send().await else { return vec![] };
        if resp.status().as_u16() == 401 { *APPLE_TOKEN.lock() = None; continue; }
        let Ok(v) = resp.json::<Value>().await else { return vec![] };
        let ids = v.pointer("/results/songs/data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
        return ids.iter().filter_map(|s| {
            let id = txt(s, "id")?;
            let a = v.pointer(&format!("/resources/songs/{id}/attributes"))?;
            Some(AppleHit { id: id.to_string(), name: txt(a, "name")?.to_string(), artist: txt(a, "artistName").unwrap_or("").to_string(), dur_ms: num(a, "durationInMillis") })
        }).collect();
    }
    vec![]
}

static PAREN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*\(.*?\)|\s*\[.*?\]").unwrap());

/// Pontuação do `scoreAndFilterResults` do MetroFuse: duração, título e artista.
fn apple_score(h: &AppleHit, title: &str, artist: &str, dur_ms: f64) -> f64 {
    let mut s = 0.0;
    if let Some(d) = h.dur_ms.filter(|_| dur_ms > 0.0) {
        let diff = (d - dur_ms).abs();
        s += if diff <= 2000.0 { 100.0 } else if diff <= 5000.0 { 50.0 } else if diff <= 10000.0 { 10.0 } else { -50.0 };
    }
    let want = PAREN.replace_all(title, "").to_lowercase().trim().to_string();
    let got = PAREN.replace_all(&h.name, "").to_lowercase().trim().to_string();
    if got == want { s += 80.0 } else if got.contains(&want) || want.contains(&got) { s += 40.0 }
    let (hn, tl) = (h.name.to_lowercase(), title.to_lowercase());
    if hn.contains("mixed") && !tl.contains("mixed") { s -= 60.0 }
    if hn.contains("remix") && !tl.contains("remix") { s -= 40.0 }
    let (ha, wa) = (h.artist.to_lowercase(), clean_artist(artist).to_lowercase());
    if ha.contains(&wa) { s += 50.0 } else if wa.split_whitespace().filter(|w| w.chars().count() > 2).any(|w| ha.contains(w)) { s += 25.0 }
    s
}

fn apple_lines(v: &Value) -> Option<Lyrics> {
    let name = "Apple Music";
    if let Some(l) = txt(v, "ttmlContent").filter(|t| !t.trim().is_empty()).map(parse_ttml).filter(|l| !l.is_empty()) { return lyrics(name, true, l); }
    for key in ["elrcMultiPerson", "elrc"] {
        if let Some(l) = txt(v, key).filter(|t| !t.trim().is_empty()).map(parse_lrc).filter(|l| !l.is_empty()) { return lyrics(name, true, l); }
    }
    if let Some(p) = txt(v, "plain").filter(|t| !t.trim().is_empty()) { return lyrics(name, false, plain_lines(p)); }
    let content = v.get("content")?.as_array()?;
    let word_level = txt(v, "type") == Some("Syllable");
    let parts = |arr: Option<&Vec<Value>>| -> Vec<(String, f64, f64)> {
        arr.map(|a| a.iter().filter_map(|w| {
            let t = txt(w, "text")?;
            let part = w.get("part").and_then(|p| p.as_bool()).unwrap_or(false);
            // `part` = sílaba que continua a palavra: não leva espaço depois
            Some((if part { t.to_string() } else { format!("{t} ") }, num(w, "timestamp")? / 1000.0, num(w, "endtime").or_else(|| num(w, "timestamp"))? / 1000.0))
        }).collect()).unwrap_or_default()
    };
    let mut lines = vec![];
    for c in content {
        let t = num(c, "timestamp").unwrap_or(0.0) / 1000.0;
        let end = num(c, "endtime").map(|e| e / 1000.0);
        let bg = c.get("background").and_then(|b| b.as_bool()).unwrap_or(false);
        let agent = Some(if c.get("oppositeTurn").and_then(|b| b.as_bool()).unwrap_or(false) { "v2" } else { "v1" }.to_string());
        let w = parts(c.get("text").and_then(|x| x.as_array()));
        if word_level {
            if let Some(l) = mk_line(join_syllables(&w), t, end, bg, agent.clone()) { lines.push(l); }
            let b = parts(c.get("backgroundText").and_then(|x| x.as_array()));
            if let Some(first) = b.first().map(|x| x.1) { if let Some(l) = mk_line(join_syllables(&b), first, None, true, None) { lines.push(l); } }
        } else {
            let text = w.iter().map(|x| x.0.trim().to_string()).collect::<Vec<_>>().join(" ");
            if text.trim().is_empty() { continue; }
            let end = end.unwrap_or(t + 3.0).max(t + 0.2);
            lines.push(Line { t, end, words: crate::lyrics::spread(&text, t, end), text, bg, agent });
        }
    }
    lyrics(name, word_level, lines)
}

pub async fn apple(t: &TrackInfo) -> Option<Lyrics> {
    let (ct, ca) = (clean_title(&t.title), clean_artist(&t.artist_line()));
    let mut queries = vec![format!("{ct} {ca}"), ct.clone()];
    if let Some(al) = t.album.as_deref().filter(|a| !a.trim().is_empty()) { queries.push(format!("{ct} {ca} {al}")); }
    let mut ranked: Vec<(AppleHit, f64)> = vec![];
    for q in queries {
        let hits = apple_search(&q).await;
        if hits.is_empty() { continue; }
        ranked = hits.into_iter().map(|h| { let s = apple_score(&h, &t.title, &t.artist_line(), t.duration_ms.unwrap_or(0) as f64); (h, s) }).filter(|(_, s)| *s > 0.0).collect();
        if !ranked.is_empty() { break; }
    }
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (h, _) in ranked.into_iter().take(10) {
        let Ok(resp) = CLIENT.get(format!("{PAX}/apple-music/lyrics")).query(&[("id", h.id.as_str())]).header("User-Agent", PAX_UA).timeout(Duration::from_secs(15)).send().await else { continue };
        if !resp.status().is_success() { continue; }
        if let Some(l) = resp.json::<Value>().await.ok().and_then(|v| apple_lines(&v)) { return Some(l); }
    }
    None
}

// ───────── Musixmatch (apic-desktop, a mesma API do app de desktop)

const MXM_APP: &str = "web-desktop-app-v1.0";
const MXM_SEED: &str = "260816fe956776e6cdfa67bb7f3d7e204ce0cf23432d1ed897d70a";
static MXM_TOKEN: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

async fn mxm_get(path: &str, q: &[(&str, String)]) -> Option<Value> {
    let r = CLIENT.get(format!("https://apic-desktop.musixmatch.com{path}")).query(q).header("User-Agent", BROWSER).header("Cookie", "x-mxm-token-guid=")
        .timeout(Duration::from_secs(15)).send().await.ok()?;
    serde_json::from_str(&r.text().await.ok()?).ok() // a API manda JSON com Content-Type text/plain
}
fn mxm_status(v: &Value) -> Option<i64> { v.pointer("/message/header/status_code").and_then(|s| s.as_i64().or_else(|| s.as_str().and_then(|x| x.parse().ok()))) }
/// `body` é `[]` (e não objeto) quando a chamada não tem dados, mesmo com status 200.
fn mxm_body(v: &Value) -> Option<&Value> { v.pointer("/message/body").filter(|b| b.is_object()) }

async fn mxm_token(fresh: bool) -> String {
    if !fresh { if let Some(t) = MXM_TOKEN.lock().clone() { return t; } }
    let got = mxm_get("/ws/1.1/token.get", &[("app_id", MXM_APP.into()), ("user_language", "en".into())]).await
        .filter(|v| mxm_status(v) == Some(200)).and_then(|v| mxm_body(&v).and_then(|b| txt(b, "user_token")).map(str::to_string)).filter(|t| !t.is_empty());
    let t = got.unwrap_or_else(|| MXM_SEED.to_string());
    *MXM_TOKEN.lock() = Some(t.clone());
    t
}

/// `track.richsync.get`: palavra a palavra. `ts`/`te` em segundos; cada palavra tem `c` (texto) e `o` (deslocamento).
fn mxm_richsync(body: &str) -> Vec<Line> {
    let Ok(Value::Array(rows)) = serde_json::from_str::<Value>(body) else { return vec![] };
    rows.iter().filter_map(|r| {
        let (ts, te) = (num(r, "ts")?, num(r, "te"));
        let ws = r.get("l")?.as_array()?;
        let items: Vec<(&str, f64)> = ws.iter().filter_map(|w| Some((txt(w, "c")?, ts + num(w, "o").unwrap_or(0.0)))).collect();
        let parts: Vec<(String, f64, f64)> = items.iter().enumerate().map(|(i, (c, s))| {
            let end = items.get(i + 1).map(|n| n.1).or(te).unwrap_or(s + 0.4).max(*s);
            (c.to_string(), *s, end)
        }).collect();
        // no richsync cada item já é uma palavra (ou um espaço solto)
        let words: Vec<Word> = parts.iter().filter(|p| !p.0.trim().is_empty()).map(|p| Word { t: p.1, end: p.2, w: p.0.trim().to_string() }).collect();
        mk_line(words, ts, te, false, None)
    }).collect()
}

/// O Musixmatch responde "algo" mesmo sem a faixa (com token de convidado chega a devolver letra de outra música);
/// só vale se o título e o artista que ele casou batem com o pedido.
fn mxm_same_track(track: &Value, title: &str, artist: &str) -> bool {
    let norm = |s: &str| s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>();
    let (wt, wa) = (norm(&clean_title(title)), norm(&clean_artist(artist)));
    let (gt, ga) = (norm(txt(track, "track_name").unwrap_or("")), norm(txt(track, "artist_name").unwrap_or("")));
    let title_ok = !gt.is_empty() && !wt.is_empty() && (gt.contains(&wt) || wt.contains(&gt));
    let artist_ok = wa.is_empty() || (!ga.is_empty() && (ga.contains(&wa) || wa.contains(&ga)));
    title_ok && artist_ok
}

async fn mxm_fetch(t: &TrackInfo, token: &str) -> (Option<Lyrics>, bool) {
    let dur = secs(t).round() as i64;
    let (artist, title) = (clean_artist(&t.artist_line()), clean_title(&t.title));
    let mut q: Vec<(&str, String)> = vec![("format", "json".into()), ("namespace", "lyrics_richsynched".into()), ("subtitle_format", "lrc".into()), ("app_id", MXM_APP.into()), ("usertoken", token.into()),
        ("q_artist", artist), ("q_track", title), ("f_subtitle_length", dur.to_string())];
    if dur > 0 { q.push(("q_duration", dur.to_string())); }
    let Some(v) = mxm_get("/ws/1.1/macro.subtitles.get", &q).await else { return (None, false) };
    if mxm_status(&v) == Some(401) { return (None, true); }
    let Some(calls) = mxm_body(&v).and_then(|b| b.get("macro_calls")) else { return (None, false) };
    let ok = |name: &str| calls.get(name).filter(|c| mxm_status(c) == Some(200));
    let force_line = crate::settings::current().values.get("musixmatchForceLineSynced").and_then(|v| v.as_bool()).unwrap_or(false);
    // letra palavra a palavra: vem em chamada própria, com o track_id que o matcher resolveu
    let track = ok("matcher.track.get").and_then(|m| mxm_body(m)).and_then(|b| b.get("track"));
    if !track.map_or(false, |tr| mxm_same_track(tr, &t.title, &t.artist_line())) { return (None, false); }
    if let (false, Some(tr)) = (force_line, track) {
        let id = num(tr, "track_id");
        if let (Some(id), true) = (id, num(tr, "has_richsync") == Some(1.0)) {
            let mut rq: Vec<(&str, String)> = vec![("app_id", MXM_APP.into()), ("usertoken", token.into()), ("track_id", (id as i64).to_string()), ("subtitle_format", "mxm".into())];
            if dur > 0 { rq.push(("f_subtitle_length", dur.to_string())); }
            if let Some(r) = mxm_get("/ws/1.1/track.richsync.get", &rq).await.filter(|r| mxm_status(r) == Some(200)) {
                let body = r.pointer("/message/body/richsync/richsync_body").and_then(|b| b.as_str()).unwrap_or("");
                let lines = mxm_richsync(body);
                if !lines.is_empty() { return (lyrics("Musixmatch", true, lines), false); }
            }
        }
    }
    let sub = ok("track.subtitles.get").and_then(|c| c.pointer("/message/body/subtitle_list/0/subtitle/subtitle_body")).and_then(|b| b.as_str()).filter(|s| !s.trim().is_empty());
    if let Some(s) = sub { let l = parse_lrc(s); if !l.is_empty() { return (lyrics("Musixmatch", true, l), false); } }
    let plain = ok("track.lyrics.get").and_then(|c| c.pointer("/message/body/lyrics/lyrics_body")).and_then(|b| b.as_str()).filter(|s| !s.trim().is_empty());
    (plain.and_then(|p| lyrics("Musixmatch", false, plain_lines(&strip_mxm_footer(p)))), false)
}

/// A letra simples do Musixmatch termina com um aviso de licença ("******* This Lyrics is NOT...").
fn strip_mxm_footer(s: &str) -> String { s.split("*******").next().unwrap_or(s).trim().to_string() }

pub async fn musixmatch(t: &TrackInfo) -> Option<Lyrics> {
    let token = mxm_token(false).await;
    let (l, unauthorized) = mxm_fetch(t, &token).await;
    if !unauthorized { return l; }
    let token = mxm_token(true).await;
    mxm_fetch(t, &token).await.0
}

// ───────── LyricsPlus (espelhos do servidor + Binimum)

const PLUS_SERVERS: [&str; 4] = ["https://lyricsplus.binimum.org", "https://lyricsplus.atomix.one", "https://lyricsplus.prjktla.my.id", "https://lyricsplus-seven.vercel.app"];
static PLUS_LAST: Lazy<Mutex<Option<usize>>> = Lazy::new(|| Mutex::new(None));

fn plus_lines(v: &Value) -> Vec<Line> {
    let word_sync = txt(v, "type").map_or(false, |t| t.eq_ignore_ascii_case("word"));
    let rows = v.get("lyrics").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    // singers: v1/v2 ficam como estão; qualquer outro vira o primeiro v1/v2 livre
    let mut map: Vec<(String, String)> = vec![];
    for r in &rows {
        let Some(raw) = r.pointer("/element/singer").and_then(|s| s.as_str()).map(str::to_lowercase) else { continue };
        if map.iter().any(|m| m.0 == raw) { continue; }
        let id = if matches!(raw.as_str(), "v1" | "v2" | "v1000") { raw.clone() } else { ["v1", "v2"].iter().find(|c| !map.iter().any(|m| m.1 == **c)).unwrap_or(&"v1").to_string() };
        map.push((raw, id));
    }
    let multi = map.len() > 1 || (map.len() == 1 && map[0].1 != "v1");
    let words_of = |arr: &[&Value]| -> Vec<(String, f64, f64)> {
        arr.iter().map(|w| { let s = num(w, "time").unwrap_or(0.0) / 1000.0; (txt(w, "text").unwrap_or("").to_string(), s, s + num(w, "duration").unwrap_or(0.0) / 1000.0) }).collect()
    };
    let mut lines = vec![];
    for r in &rows {
        let t = num(r, "time").unwrap_or(0.0) / 1000.0;
        let end = num(r, "duration").map(|d| t + d / 1000.0).filter(|e| *e > t);
        let syl: Vec<&Value> = r.get("syllabus").and_then(|s| s.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
        let is_bg = |w: &&Value| w.get("isBackground").and_then(|b| b.as_bool()).unwrap_or(false);
        let main: Vec<&Value> = syl.iter().copied().filter(|w| !is_bg(w)).collect();
        let back: Vec<&Value> = syl.iter().copied().filter(is_bg).collect();
        let text = txt(r, "text").unwrap_or("").trim().to_string();
        let agent = r.pointer("/element/singer").and_then(|s| s.as_str()).map(str::to_lowercase).and_then(|raw| map.iter().find(|m| m.0 == raw).map(|m| m.1.clone())).filter(|_| multi);
        if word_sync && !main.is_empty() {
            if let Some(l) = mk_line(join_syllables(&words_of(&main)), t, end, false, agent) { lines.push(l); }
        } else if !(main.is_empty() && !back.is_empty()) && !text.is_empty() {
            let e = end.unwrap_or(t + 3.0);
            lines.push(Line { t, end: e, words: crate::lyrics::spread(&text, t, e), text: text.clone(), bg: false, agent });
        }
        if !back.is_empty() {
            let b = words_of(&back);
            let start = b.iter().map(|x| x.1).fold(f64::INFINITY, f64::min);
            if word_sync { if let Some(l) = mk_line(join_syllables(&b), start, None, true, None) { lines.push(l); } }
            else if !text.is_empty() { lines.push(Line { t: start, end: start + 2.0, words: crate::lyrics::spread(&text, start, start + 2.0), text, bg: true, agent: None }); }
        }
    }
    lines
}

async fn plus_server(t: &TrackInfo) -> Option<Lyrics> {
    let (title, artist) = (t.title.clone(), t.artist_line());
    if title.trim().is_empty() || artist.trim().is_empty() { return None; }
    let first = PLUS_LAST.lock().unwrap_or(0);
    let order: Vec<usize> = std::iter::once(first).chain((0..PLUS_SERVERS.len()).filter(|i| *i != first)).collect();
    for i in order {
        let mut q: Vec<(&str, String)> = vec![("title", title.clone()), ("artist", artist.clone())];
        if let Some(d) = t.duration_ms.filter(|d| *d > 0) { q.push(("duration", (d / 1000).to_string())); }
        if let Some(a) = t.album.clone().filter(|a| !a.trim().is_empty()) { q.push(("album", a)); }
        let Ok(r) = CLIENT.get(format!("{}/v2/lyrics/get", PLUS_SERVERS[i])).query(&q).header("User-Agent", PAX_UA).timeout(Duration::from_secs(12)).send().await else { continue };
        if !r.status().is_success() { continue; }
        let Ok(v) = r.json::<Value>().await else { continue };
        let lines = plus_lines(&v);
        if !lines.is_empty() {
            *PLUS_LAST.lock() = Some(i);
            let word = txt(&v, "type").map_or(false, |t| t.eq_ignore_ascii_case("word"));
            return lyrics("LyricsPlus", word || lines.iter().any(|l| l.t > 0.0), lines);
        }
    }
    None
}

/// Binimum: busca por faixa e devolve o TTML (palavra a palavra quando `timing_type == "word"`).
async fn binimum(t: &TrackInfo) -> Option<(Lyrics, bool)> {
    let mut q: Vec<(&str, String)> = vec![("track", clean_title(&t.title)), ("artist", clean_artist(&t.artist_line()))];
    if let Some(a) = t.album.clone().filter(|a| !a.trim().is_empty()) { q.push(("album", a)); }
    if let Some(d) = t.duration_ms.filter(|d| *d > 0) { q.push(("duration", (d / 1000).to_string())); }
    let isrc = t.isrc.clone().filter(|i| Regex::new(r"^[A-Z]{2}[A-Z0-9]{3}\d{7}$").unwrap().is_match(&i.to_uppercase()));
    let mut attempts: Vec<Vec<(&str, String)>> = vec![];
    if let Some(i) = isrc { attempts.push(vec![("isrc", i.to_uppercase())]); }
    attempts.push(q);
    for a in attempts {
        let Ok(r) = CLIENT.get("https://lyrics-api.binimum.org/").query(&a).header("User-Agent", PAX_UA).timeout(Duration::from_secs(12)).send().await else { continue };
        if !r.status().is_success() { continue; }
        let Ok(v) = r.json::<Value>().await else { continue };
        let Some(hit) = v.get("results").and_then(|r| r.as_array()).and_then(|r| r.iter().find(|x| txt(x, "lyricsUrl").map_or(false, |u| !u.is_empty()))) else { continue };
        let Ok(resp) = CLIENT.get(txt(hit, "lyricsUrl").unwrap()).header("User-Agent", PAX_UA).timeout(Duration::from_secs(12)).send().await else { continue };
        if !resp.status().is_success() { continue; }
        let Ok(ttml) = resp.text().await else { continue };
        let lines = parse_ttml(&ttml);
        if lines.is_empty() { continue; }
        let word = txt(hit, "timing_type").map_or(false, |t| t.eq_ignore_ascii_case("word"));
        return Some((Lyrics { synced: true, provider: "Binimum".into(), lines }, word));
    }
    None
}

/// Provedor "Binimum" da lista.
pub async fn bini(t: &TrackInfo) -> Option<Lyrics> { binimum(t).await.map(|(l, _)| l) }

/// Provedor "LyricsPlus" da lista: o Binimum vale se tem palavra a palavra; senão fica o melhor entre os dois.
pub async fn plus(t: &TrackInfo) -> Option<Lyrics> {
    let bini = binimum(t).await;
    if let Some((l, true)) = &bini { return Some(Lyrics { provider: "LyricsPlus".into(), ..l.clone() }); }
    let server = plus_server(t).await;
    let word_server = server.as_ref().map_or(false, |s| s.lines.iter().any(|l| l.words.len() > 1 && l.words.iter().any(|w| w.end - w.t > 0.0)) && s.synced);
    match (server, bini) {
        (Some(s), _) if word_server => Some(s),
        (_, Some((b, _))) => Some(Lyrics { provider: "LyricsPlus".into(), ..b }),
        (s, None) => s,
    }
}

// ───────── QQ Music (API pública da web; blob QRC cifrado)

static QRC_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[(\d+),(\d+)\](.*)$").unwrap());
static QRC_WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"([^(]*)\((\d+),(\d+)\)").unwrap());
static QRC_CONTENT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?s)<Lyric_1[^>]*LyricContent="(.*?)"\s*/?>"#).unwrap());

fn unescape_xml(s: &str) -> String { s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&#10;", "\n").replace("&#13;", "").replace("&amp;", "&") }

/// Converte o texto decifrado: QRC (`[ini,dur]palavra(ini,dur)...`) vira linhas com palavras; LRC comum passa direto.
pub fn parse_qrc(decoded: &str) -> Vec<Line> {
    let body = QRC_CONTENT.captures(decoded).map(|c| unescape_xml(&c[1])).unwrap_or_else(|| decoded.to_string());
    if !Regex::new(r"\(\d+,\d+\)").unwrap().is_match(&body) { return parse_lrc(&body); }
    body.lines().filter_map(|raw| {
        let m = QRC_LINE.captures(raw.trim())?;
        let (start, dur) = (m[1].parse::<f64>().ok()? / 1000.0, m[2].parse::<f64>().ok()? / 1000.0);
        let parts: Vec<(String, f64, f64)> = QRC_WORD.captures_iter(&m[3]).filter_map(|w| {
            let (s, d) = (w[2].parse::<f64>().ok()? / 1000.0, w[3].parse::<f64>().ok()? / 1000.0);
            Some((w[1].to_string(), s, s + d))
        }).collect();
        // sílabas latinas se juntam; ideogramas e palavras separadas por espaço ficam como estão
        let latin = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '\'' || c == '-');
        let mut words: Vec<Word> = vec![];
        let mut glue = false;
        for (text, s, e) in &parts {
            let w = text.trim();
            if w.is_empty() { glue = false; continue; }
            if glue && !text.starts_with(char::is_whitespace) && latin(w) && words.last().map_or(false, |l| latin(&l.w)) {
                let last = words.last_mut().unwrap(); last.w.push_str(w); last.end = *e;
            } else { words.push(Word { t: *s, end: *e, w: w.to_string() }); }
            glue = !text.ends_with(char::is_whitespace);
        }
        mk_line(words, start, Some(start + dur).filter(|e| *e > start), false, None)
    }).collect()
}

fn qq_decode(hex_blob: &str) -> Option<String> {
    use std::io::Read;
    let clean: Vec<u8> = hex_blob.bytes().filter(u8::is_ascii_hexdigit).collect();
    let bytes: Vec<u8> = clean.chunks(2).filter(|c| c.len() == 2).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect();
    let s1 = crate::qqdes::crypt(&bytes, b"!@#)(NHLiuy*$%^&", true);
    let s2 = crate::qqdes::crypt(&s1, b"123ZXC!@#)(*$%^&", false);
    let s3 = crate::qqdes::crypt(&s2, b"!@#)(*$%^&abcDEF", true);
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&s3[..]).read_to_end(&mut out).ok()?;
    String::from_utf8(out).ok()
}

pub async fn qq(t: &TrackInfo) -> Option<Lyrics> {
    let (ct, ca) = (clean_title(&t.title), clean_artist(&t.artist_line()));
    let v: Value = CLIENT.get("https://c.y.qq.com/soso/fcgi-bin/client_search_cp").query(&[("w", format!("{ct} {ca}")), ("format", "json".into()), ("p", "1".into()), ("n", "20".into())])
        .header("Referer", "https://y.qq.com/").header("User-Agent", BROWSER).timeout(Duration::from_secs(10)).send().await.ok()?.json().await.ok()?;
    let list = v.pointer("/data/song/list")?.as_array()?;
    let want = ct.to_lowercase();
    let dur = secs(t);
    let best = list.iter().max_by_key(|s| {
        let name = txt(s, "songname").or_else(|| txt(s, "name")).unwrap_or("").to_lowercase();
        let mut score = if name.trim() == want { 80 } else if !name.is_empty() && (name.contains(&want) || want.contains(name.trim())) { 40 } else { 0 };
        if let (Some(d), true) = (num(s, "interval").or_else(|| num(s, "duration")), dur > 0.0) {
            score += match (d - dur).abs() { x if x <= 2.0 => 100, x if x <= 5.0 => 50, x if x <= 10.0 => 10, _ => -50 };
        }
        score
    })?;
    let id = num(best, "songid").or_else(|| num(best, "id"))? as i64;
    let xml = CLIENT.get("https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg").query(&[("version", "15"), ("miniversion", "82"), ("lrctype", "4"), ("musicid", &id.to_string())])
        .header("Referer", "https://y.qq.com/").header("User-Agent", BROWSER).timeout(Duration::from_secs(10)).send().await.ok()?.text().await.ok()?
        .replace("<!--", "").replace("-->", "");
    let block = Regex::new(r"(?s)<content[^>]*>(.*?)</content>").unwrap().captures(&xml)?[1].trim().to_string();
    // o bloco vem em CDATA; "CDATA" tem letras hexadecimais e contaminaria os bytes se não fosse removido antes
    let hex = Regex::new(r"(?s)<!\[CDATA\[(.*?)\]\]>").unwrap().captures(&block).map(|c| c[1].trim().to_string()).unwrap_or(block);
    let lines = parse_qrc(&qq_decode(&hex)?);
    lyrics("QQ Music", true, drop_credits(lines))
}

// ───────── YouTube Music (texto simples)

pub async fn youtube(t: &TrackInfo) -> Option<Lyrics> {
    let id = if t.provider == "youtube" { t.id.clone() } else {
        let found = crate::providers::youtube::search(&format!("{} {}", t.title, clean_artist(&t.artist_line())), 1).await.ok()?;
        found.first()?.id.clone()
    };
    let text = crate::providers::ytmusic::lyrics_text(&id).await.ok()??;
    lyrics("YouTube Music", false, plain_lines(&text))
}

/// Legenda do vídeo (YouTubeSubtitle do MetroFuse): linhas com tempo, sem palavra a palavra.
pub async fn ytsub(t: &TrackInfo) -> Option<Lyrics> {
    let id = if t.provider == "youtube" { t.id.clone() } else {
        crate::providers::youtube::search(&format!("{} {}", t.title, clean_artist(&t.artist_line())), 1).await.ok()?.first()?.id.clone()
    };
    let cues = crate::providers::ytmusic::transcript(&id, &format!("{} {}", t.title, t.artist_line())).await.ok()?;
    let lrc: String = cues.iter().map(|(ms, text)| format!("[{:02}:{:02}.{:02}]{}\n", ms / 60000, (ms / 1000) % 60, (ms % 1000) / 10, text)).collect();
    lyrics("YouTube Legenda", true, parse_lrc(&lrc))
}

// ───────── Deezer (letra da conta, palavra a palavra; exige o arl)

const DZ_QUERY: &str = "query SynchronizedTrackLyrics($trackId: String!) { track(trackId: $trackId) { id lyrics { id text synchronizedLines { line lineTranslated milliseconds duration } synchronizedWordByWordLines { start end words { start end word } } } } }";

pub fn deezer_lines(v: &Value) -> Option<Lyrics> {
    let ly = v.pointer("/data/track/lyrics").filter(|l| !l.is_null())?;
    if let Some(rows) = ly.get("synchronizedWordByWordLines").and_then(|r| r.as_array()).filter(|r| !r.is_empty()) {
        let lines: Vec<Line> = rows.iter().filter_map(|r| {
            let words: Vec<Word> = r.get("words")?.as_array()?.iter().filter_map(|w| Some(Word { t: num(w, "start")? / 1000.0, end: num(w, "end")? / 1000.0, w: txt(w, "word")?.trim().to_string() })).collect();
            mk_line(words, num(r, "start")? / 1000.0, num(r, "end").map(|e| e / 1000.0), false, None)
        }).collect();
        if !lines.is_empty() { return lyrics("Deezer", true, lines); }
    }
    if let Some(rows) = ly.get("synchronizedLines").and_then(|r| r.as_array()).filter(|r| !r.is_empty()) {
        let mut lines: Vec<Line> = rows.iter().filter_map(|r| {
            let (text, t) = (txt(r, "line")?.trim().to_string(), num(r, "milliseconds")? / 1000.0);
            if text.is_empty() { return None; }
            let end = num(r, "duration").filter(|d| *d > 0.0).map(|d| t + d / 1000.0).unwrap_or(t + 4.0);
            Some(Line { t, end, words: crate::lyrics::spread(&text, t, end), text, bg: false, agent: None })
        }).collect();
        lines.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
        if !lines.is_empty() { return lyrics("Deezer", true, lines); }
    }
    let plain = txt(ly, "text").filter(|t| !t.trim().is_empty())?;
    lyrics("Deezer", false, plain_lines(plain))
}

async fn deezer_jwt(arl: &str) -> Option<String> {
    let v: Value = CLIENT.post("https://auth.deezer.com/login/arl").query(&[("jo", "p"), ("rto", "c"), ("i", "c")]).header("Cookie", format!("arl={arl}")).header("User-Agent", crate::http::BROWSER_UA)
        .header(reqwest::header::CONTENT_LENGTH, "0").body("").timeout(Duration::from_secs(10)).send().await.ok()?.json().await.ok()?;
    txt(&v, "jwt").map(str::to_string)
}

pub async fn deezer(t: &TrackInfo) -> Option<Lyrics> {
    if !crate::settings::current().values.get("deezerLyrics").and_then(|v| v.as_bool()).unwrap_or(true) { return None; }
    let arl = crate::providers::deezer::arl()?;
    let id = if t.provider == "deezer" { t.id.clone() } else {
        let found = crate::providers::deezer::search(&format!("{} {}", clean_title(&t.title), clean_artist(&t.artist_line())), 5).await.ok()?;
        let dur = secs(t);
        found.iter().find(|c| dur <= 0.0 || c.duration_ms.map_or(true, |d| (d as f64 / 1000.0 - dur).abs() <= 6.0))?.id.clone()
    };
    let jwt = deezer_jwt(&arl).await?;
    let v: Value = CLIENT.post("https://pipe.deezer.com/api").header("Authorization", format!("Bearer {jwt}")).header("User-Agent", crate::http::BROWSER_UA)
        .json(&serde_json::json!({ "operationName": "SynchronizedTrackLyrics", "variables": { "trackId": id }, "query": DZ_QUERY })).timeout(Duration::from_secs(12)).send().await.ok()?.json().await.ok()?;
    deezer_lines(&v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn musixmatch_so_aceita_a_mesma_faixa() {
        let nokia = json!({ "track_name": "NOKIA", "artist_name": "Drake" });
        assert!(!mxm_same_track(&nokia, "Blinding Lights", "The Weeknd"));
        let ok = json!({ "track_name": "Blinding Lights", "artist_name": "The Weeknd" });
        assert!(mxm_same_track(&ok, "Blinding Lights (Official Video)", "The Weeknd, Daft Punk"));
    }

    #[test]
    fn musixmatch_richsync() {
        let body = r#"[{"ts":10.0,"te":13.0,"l":[{"c":"Hello","o":0.0},{"c":" ","o":0.5},{"c":"world","o":1.0}],"x":"Hello world"}]"#;
        let l = mxm_richsync(body);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].text, "Hello world");
        assert_eq!(l[0].words[1].t, 11.0);
        assert_eq!(l[0].words[1].end, 13.0);
        assert_eq!(strip_mxm_footer("a\nb\n\n******* This Lyrics is NOT for Commercial use *******"), "a\nb");
    }

    #[test]
    fn lyricsplus_palavras_e_apoio() {
        let v = json!({ "type": "Word", "lyrics": [
            { "time": 1000, "duration": 3000, "text": "Hel lo world", "element": { "singer": "v1" }, "syllabus": [
                { "time": 1000, "duration": 500, "text": "Hel" }, { "time": 1500, "duration": 500, "text": "lo " }, { "time": 2000, "duration": 800, "text": "world" },
                { "time": 2900, "duration": 300, "text": "(ooh)", "isBackground": true } ] },
            { "time": 5000, "duration": 2000, "text": "Next", "element": { "singer": "v2" }, "syllabus": [ { "time": 5000, "duration": 2000, "text": "Next" } ] } ] });
        let l = plus_lines(&v);
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].text, "Hello world");
        assert_eq!(l[0].agent.as_deref(), Some("v1"));
        assert!(l[1].bg && l[1].text == "(ooh)");
        assert_eq!(l[2].agent.as_deref(), Some("v2"));
    }

    #[test]
    fn apple_conteudo_silabas() {
        let v = json!({ "type": "Syllable", "content": [
            { "timestamp": 1000, "endtime": 3000, "oppositeTurn": true, "text": [
                { "text": "Hel", "timestamp": 1000, "endtime": 1500, "part": true }, { "text": "lo", "timestamp": 1500, "endtime": 2000 }, { "text": "you", "timestamp": 2000, "endtime": 3000 } ],
              "backgroundText": [ { "text": "ah", "timestamp": 2500, "endtime": 2900 } ] } ] });
        let l = apple_lines(&v).unwrap().lines;
        assert_eq!(l[0].text, "Hello you");
        assert_eq!(l[0].agent.as_deref(), Some("v2"));
        assert_eq!(l[0].words.len(), 2);
        assert!(l[1].bg);
        let plain = apple_lines(&json!({ "plain": "um\ndois" })).unwrap();
        assert!(!plain.synced && plain.lines.len() == 2);
    }

    #[test]
    fn legenda_do_youtube() {
        let xml = "<?xml version=\"1.0\"?><timedtext format=\"3\"><body><p t=\"1360\" d=\"1680\">[\u{266a}\u{266a}\u{266a}]</p><p t=\"18640\" d=\"3240\">\u{266a} We&#39;re no strangers to love</p><p t=\"22000\" d=\"900\"><s>You</s><s t=\"300\"> know</s></p><p t=\"24000\">\n</p></body></timedtext>";
        let tagged = "<transcript><text start=\"5\" dur=\"2\">&amp;lt;font color=&amp;quot;#A0AAB4&amp;quot;&amp;gt;&amp;lt;/font&amp;gt;&amp;lt;font color=&amp;quot;485883&amp;quot;&amp;gt;&amp;lt;b&amp;gt;Yu&amp;lt;/b&amp;gt;&amp;lt;/font&amp;gt; kata</text></transcript>";
        assert_eq!(crate::providers::ytmusic::parse_timedtext(tagged), vec![(5000, "Yu kata".to_string())]);
        let std_fmt = "<transcript><text start=\"1.36\" dur=\"1\">[\u{266a}]</text><text start=\"18.64\" dur=\"3\">\u{266a} We&amp;#39;re here \u{266a}</text></transcript>";
        assert_eq!(crate::providers::ytmusic::parse_timedtext(std_fmt), vec![(18640, "We're here".to_string())]);
        assert_eq!(crate::providers::ytmusic::parse_timedtext(xml), vec![(18640, "We're no strangers to love".to_string()), (22000, "You know".to_string())]);
    }

    #[test]
    fn legenda_animada_sem_repeticao_e_sem_invisiveis() {
        let zw = "\u{200B}\u{200B}\u{200B} \u{200B}";
        let xml = format!("<transcript><text start=\"0.001\" dur=\"1\">Aviso de luzes piscantes</text><text start=\"16.734\">{zw}Yu!{zw}</text><text start=\"16.734\">{zw}Yu!{zw}</text><text start=\"17.101\">{zw}Dr. Kidori{zw}</text><text start=\"17.101\">{zw}Dr. Kidori{zw}</text><text start=\"17.868\">{zw}Dr. Kidori{zw}</text><text start=\"18.369\">{zw}Dr. Kidori{zw}</text><text start=\"20.0\">outro verso</text><text start=\"30.0\">Yu!</text><text start=\"33.0\">Yu!</text></transcript>");
        let c = crate::providers::ytmusic::parse_timedtext(&xml);
        assert_eq!(c.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(), vec!["Yu!", "Dr. Kidori", "outro verso", "Yu!", "Yu!"]);
        assert_eq!(c[1].0, 17101);
    }

    #[test]
    fn escolhe_a_legenda_no_idioma_original() {
        use crate::providers::ytmusic::pick_track;
        let t = |l: &str, asr: bool| (l.to_string(), asr);
        // o vídeo do IGAKU: en, en-GB, ja, pt, es-419 e uma automática (errada) em coreano
        let igaku = vec![t("es-419", false), t("pt", false), t("ko", true), t("en", false), t("ja", false), t("en-GB", false)];
        assert_eq!(pick_track(&igaku, "IGAKU イガク - Kasane Teto sidoopa"), Some(4)); // ja pela escrita do título
        assert_eq!(pick_track(&igaku, "IGAKU - Kasane Teto"), Some(3)); // sem pista de escrita: a automática (ko) não tem manual, vai o inglês
        let es = vec![t("en", false), t("es", false), t("es", true)];
        assert_eq!(pick_track(&es, "La Bamba"), Some(1)); // a automática diz espanhol e há manual em espanhol
        assert_eq!(pick_track(&[t("de", true)], "x"), Some(0)); // só automática
        assert_eq!(pick_track(&[], "x"), None);
    }

    #[test]
    fn qrc_palavras() {
        let q = "[ti:x]\n[1000,2000]Hel(1000,300)lo (1300,300) wor(1600,200)ld(1800,200)\n[5000,1000]你(5000,300)好(5300,300)\n";
        let l = parse_qrc(q);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].text, "Hello world");
        assert_eq!(l[0].words[1].t, 1.6);
        assert_eq!(l[1].words.len(), 2);
        assert_eq!(parse_qrc("[00:01.00]texto comum").len(), 1);
    }

    #[test]
    fn deezer_palavra_a_palavra_e_linhas() {
        let w = json!({ "data": { "track": { "lyrics": { "synchronizedWordByWordLines": [ { "start": 1000, "end": 2000, "words": [ { "start": 1000, "end": 1400, "word": "oi " }, { "start": 1400, "end": 2000, "word": "mundo" } ] } ] } } } });
        let l = deezer_lines(&w).unwrap();
        assert_eq!(l.lines[0].text, "oi mundo");
        let s = json!({ "data": { "track": { "lyrics": { "synchronizedLines": [ { "line": "b", "milliseconds": 5000, "duration": 1000 }, { "line": "a", "milliseconds": 1000, "duration": 1000 } ] } } } });
        assert_eq!(deezer_lines(&s).unwrap().lines[0].text, "a");
        assert!(deezer_lines(&json!({ "data": { "track": { "lyrics": null } } })).is_none());
    }
}
