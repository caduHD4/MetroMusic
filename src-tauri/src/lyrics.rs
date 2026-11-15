//! Letras sincronizadas. Porte de `LrcLib` e `BetterLyrics`/`TTMLParser` (MetroFuse).
//!
//! - LrcLib: letra por linha (LRC) ou, na falta dela, sem sincronia.
//! - BetterLyrics: TTML com tempo por palavra e vozes de apoio.
//! Os demais provedores (Apple Music, Musixmatch, QQ, LyricsPlus, Binimum, YouTube, Deezer) estão em `lyrics_more`.
//! A ordem vem das configurações.

use crate::http::{err, CLIENT};
use crate::model::{Res, TrackInfo};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub const SUPPORTED: [&str; 12] = ["better", "apple", "lrclib", "musixmatch", "deezer", "spotify", "qq", "kugou", "bini", "plus", "youtube", "ytsub"];
const UA: &str = "MetroMusic/0.1 (player de música desktop)";

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Word { pub t: f64, pub end: f64, pub w: String }

#[derive(Serialize, Clone, Debug)]
pub struct Line {
    pub t: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<Word>,
    pub bg: bool,
    pub agent: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Lyrics { pub synced: bool, pub provider: String, pub lines: Vec<Line> }

static CACHE: Lazy<Mutex<HashMap<String, Option<Lyrics>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Distribui o tempo da linha entre as palavras, proporcional ao tamanho (letra sem tempo por palavra).
pub(crate) fn spread(text: &str, t: f64, end: f64) -> Vec<Word> {
    let ws: Vec<&str> = text.split_whitespace().collect();
    let total: usize = ws.iter().map(|w| w.chars().count() + 1).sum::<usize>().max(1);
    let mut at = t;
    ws.iter().map(|w| {
        let d = (end - t) * (w.chars().count() + 1) as f64 / total as f64;
        let word = Word { t: at, end: at + d, w: w.to_string() };
        at += d;
        word
    }).collect()
}

// ───────── LRC

static LRC_TIME: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]").unwrap());
static LRC_WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"<\d{1,3}:\d{2}(?:[.:]\d{1,3})?>").unwrap());

/// Lê um arquivo LRC (`[mm:ss.xx] texto`, com várias marcas por linha e tags de palavra opcionais).
pub fn parse_lrc(lrc: &str) -> Vec<Line> {
    let mut raw: Vec<(f64, String)> = vec![];
    for line in lrc.lines() {
        let stamps: Vec<f64> = LRC_TIME.captures_iter(line).map(|c| {
            let frac = c.get(3).map(|f| { let s = f.as_str(); s.parse::<f64>().unwrap_or(0.0) / 10f64.powi(s.len() as i32) }).unwrap_or(0.0);
            c[1].parse::<f64>().unwrap_or(0.0) * 60.0 + c[2].parse::<f64>().unwrap_or(0.0) + frac
        }).collect();
        if stamps.is_empty() { continue; } // metadados ([ar:], [ti:]...)
        let text = LRC_WORD.replace_all(&LRC_TIME.replace_all(line, ""), "").trim().to_string();
        for s in stamps { raw.push((s, text.clone())); }
    }
    raw.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = raw.len();
    (0..n).map(|i| {
        let (t, text) = &raw[i];
        let next = raw.get(i + 1).map(|r| r.0).unwrap_or(t + 5.0);
        // uma linha dura até a próxima, no máximo 7 s (depois disso é pausa instrumental)
        let end = (next - 0.08).clamp(t + 0.6, t + 7.0).max(*t + 0.3);
        Line { t: *t, end, words: spread(text, *t, end), text: text.clone(), bg: false, agent: None }
    }).filter(|l| !l.text.is_empty()).collect()
}

pub fn plain_lines(text: &str) -> Vec<Line> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| Line { t: 0.0, end: 0.0, text: l.to_string(), words: vec![], bg: false, agent: None }).collect()
}

// ───────── LrcLib

static CLEAN: Lazy<Vec<Regex>> = Lazy::new(|| {
    let kw = "official|video|audio|lyrics|lyric|visualizer|hd|hq|4k|remaster|remix|live|acoustic|version|edit|extended|radio|clean|explicit";
    [
        format!(r"(?i)\s*\(.*?({kw}).*?\)"), format!(r"(?i)\s*\[.*?({kw}).*?\]"), r"\s*【.*?】".to_string(), r"\s*\|.*$".to_string(),
        r"(?i)\s*-\s*(official|video|audio|lyrics|lyric|visualizer).*$".to_string(), r"(?i)\s*\(feat\..*?\)".to_string(), r"(?i)\s*\(ft\..*?\)".to_string(),
        r"(?i)\s*feat\..*$".to_string(), r"(?i)\s*ft\..*$".to_string(),
    ].iter().map(|p| Regex::new(p).unwrap()).collect()
});

pub(crate) fn clean_title(t: &str) -> String { CLEAN.iter().fold(t.trim().to_string(), |acc, re| re.replace_all(&acc, "").to_string()).trim().to_string() }

pub(crate) fn clean_artist(a: &str) -> String {
    let a = a.trim();
    for sep in [" & ", " and ", ", ", " x ", " feat. ", " feat ", " ft. ", " ft ", " featuring ", " with "] {
        if let Some(i) = a.to_lowercase().find(sep) { return a[..i].trim().to_string(); }
    }
    a.to_string()
}

#[derive(Clone)]
struct LrcTrack { duration: f64, synced: Option<String>, plain: Option<String> }

async fn lrc_query(params: &[(&str, &str)]) -> Vec<LrcTrack> {
    let Ok(resp) = CLIENT.get("https://lrclib.net/api/search").query(params).header("User-Agent", UA).timeout(Duration::from_secs(12)).send().await else { return vec![] };
    let Ok(list) = resp.json::<Vec<Value>>().await else { return vec![] };
    list.iter().filter_map(|v| {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|x| !x.trim().is_empty()).map(str::to_string);
        let (synced, plain) = (s("syncedLyrics"), s("plainLyrics"));
        if synced.is_none() && plain.is_none() { return None; }
        Some(LrcTrack { duration: v.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0), synced, plain })
    }).collect()
}

/// As cinco estratégias de busca do MetroFuse, da mais específica à mais ampla.
async fn lrc_search(title: &str, artist: &str, album: Option<&str>) -> Vec<LrcTrack> {
    let (ct, ca) = (clean_title(title), clean_artist(artist));
    let mut attempts: Vec<Vec<(&str, String)>> = vec![];
    let mut a1 = vec![("track_name", ct.clone()), ("artist_name", ca.clone())];
    if let Some(al) = album { a1.push(("album_name", al.to_string())); }
    attempts.push(a1);
    attempts.push(vec![("track_name", ct.clone())]);
    attempts.push(vec![("q", format!("{ca} {ct}"))]);
    attempts.push(vec![("q", ct.clone())]);
    if ct != title.trim() { attempts.push(vec![("track_name", title.trim().to_string()), ("artist_name", artist.trim().to_string())]); }
    for a in attempts {
        let p: Vec<(&str, &str)> = a.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let r = lrc_query(&p).await;
        if !r.is_empty() { return r; }
    }
    vec![]
}

async fn lrclib(t: &TrackInfo) -> Option<Lyrics> {
    let found = lrc_search(&t.title, &t.artist_line(), t.album.as_deref()).await;
    let want = t.duration_ms.map(|d| d as f64 / 1000.0).filter(|d| *d > 0.0);
    // com duração conhecida, aceita até 5 s de diferença; prefere a que tem sincronia
    let mut ok: Vec<&LrcTrack> = found.iter().filter(|c| want.map_or(true, |w| c.duration <= 0.0 || (c.duration - w).abs() <= 5.0)).collect();
    ok.sort_by(|a, b| (b.synced.is_some()).cmp(&a.synced.is_some()).then_with(|| {
        let da = want.map_or(0.0, |w| (a.duration - w).abs()); let db = want.map_or(0.0, |w| (b.duration - w).abs());
        da.partial_cmp(&db).unwrap()
    }));
    let best = ok.first()?;
    if let Some(s) = &best.synced {
        let lines = parse_lrc(s);
        if !lines.is_empty() { return Some(Lyrics { synced: true, provider: "LrcLib".into(), lines }); }
    }
    let lines = plain_lines(best.plain.as_deref()?);
    (!lines.is_empty()).then(|| Lyrics { synced: false, provider: "LrcLib".into(), lines })
}

// ───────── KuGou (LRC por linha, API pública)

/// Remove as linhas de crédito que o KuGou coloca no começo (título, letrista, compositor).
pub(crate) fn drop_credits(lines: Vec<Line>) -> Vec<Line> {
    const MARKS: [&str; 8] = ["作词", "作曲", "编曲", "制作", "Lyrics by", "Composed by", "Written by", "Produced by"];
    lines.into_iter().enumerate().filter(|(i, l)| !(*i < 8 && l.t < 20.0 && (MARKS.iter().any(|m| l.text.contains(m)) || l.text.contains('：') || (*i == 0 && l.text.contains(" - "))))).map(|(_, l)| l).collect()
}

fn decode_kugou(content_b64: &str) -> Option<Vec<Line>> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD.decode(content_b64.trim()).ok()?;
    let text = String::from_utf8(raw).ok()?;
    let lines = drop_credits(parse_lrc(text.trim_start_matches('\u{feff}')));
    (!lines.is_empty()).then_some(lines)
}

async fn kugou(t: &TrackInfo) -> Option<Lyrics> {
    let dur = t.duration_ms.unwrap_or(0);
    let keyword = format!("{} - {}", clean_artist(&t.artist_line()), clean_title(&t.title));
    let search: Value = CLIENT.get("https://lyrics.kugou.com/search")
        .query(&[("ver", "1"), ("man", "yes"), ("client", "pc"), ("keyword", keyword.as_str()), ("duration", &dur.to_string())])
        .header("User-Agent", UA).timeout(Duration::from_secs(10)).send().await.ok()?.json().await.ok()?;
    let candidates = search.get("candidates")?.as_array()?;
    // com duração conhecida, aceita até 5 s de diferença
    let pick = candidates.iter().find(|c| dur == 0 || c.get("duration").and_then(|d| d.as_u64()).map_or(true, |d| d == 0 || d.abs_diff(dur) <= 5000))?;
    let (id, key) = (pick.get("id")?.as_str()?, pick.get("accesskey")?.as_str()?);
    let dl: Value = CLIENT.get("https://lyrics.kugou.com/download")
        .query(&[("ver", "1"), ("client", "pc"), ("id", id), ("accesskey", key), ("fmt", "lrc"), ("charset", "utf8")])
        .header("User-Agent", UA).timeout(Duration::from_secs(10)).send().await.ok()?.json().await.ok()?;
    let lines = decode_kugou(dl.get("content")?.as_str()?)?;
    Some(Lyrics { synced: true, provider: "KuGou".into(), lines })
}

// ───────── Spotify (letra do player web; precisa da conta conectada)

async fn spotify(t: &TrackInfo) -> Option<Lyrics> {
    if !crate::settings::current().values.get("spotifyLyrics").and_then(|v| v.as_bool()).unwrap_or(true) { return None; }
    let id = crate::providers::spotify::resolve_id(t).await.ok()??;
    let (synced, text) = crate::providers::spotify::color_lyrics(&id).await.ok()??;
    let lines = if synced { parse_lrc(&text) } else { plain_lines(&text) };
    (!lines.is_empty()).then(|| Lyrics { synced, provider: "Spotify".into(), lines })
}

// ───────── BetterLyrics (TTML)

fn parse_time(s: &str) -> f64 {
    let s = s.trim().trim_end_matches('s');
    let parts: Vec<f64> = s.split(':').map(|p| p.parse::<f64>().unwrap_or(0.0)).collect();
    match parts.as_slice() { [h, m, sec] => h * 3600.0 + m * 60.0 + sec, [m, sec] => m * 60.0 + sec, [sec] => *sec, _ => 0.0 }
}

const NS_META: &str = "http://www.w3.org/ns/ttml#metadata";

fn attr<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    n.attribute((NS_META, name)).or_else(|| n.attribute(name)).filter(|v| !v.is_empty())
}
fn timing<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    n.attribute(name).or_else(|| n.attribute(("http://www.w3.org/ns/ttml#parameter", name))).filter(|v| !v.is_empty())
}

struct Span { text: String, start: f64, end: f64, space: bool }

fn word_span(n: roxmltree::Node, offset: f64, out: &mut Vec<Span>) {
    let (Some(b), Some(e)) = (timing(n, "begin"), timing(n, "end")) else { return };
    let text: String = n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect();
    let next_space = n.next_sibling().filter(|s| s.is_text()).and_then(|s| s.text()).map_or(false, |t| t.starts_with(char::is_whitespace));
    let space = text.ends_with(char::is_whitespace) || next_space;
    out.push(Span { text, start: parse_time(b) + offset, end: parse_time(e) + offset, space });
}

/// Junta sílabas em palavras: só quebra onde há espaço e a sílaba não termina em hífen.
fn merge(spans: &[Span]) -> Vec<Word> {
    if spans.is_empty() { return vec![]; }
    let mut words = vec![];
    let (mut text, mut start, mut end) = (spans[0].text.clone(), spans[0].start, spans[0].end);
    for i in 1..spans.len() {
        let (prev, cur) = (&spans[i - 1], &spans[i]);
        if prev.space && !prev.text.ends_with('-') {
            words.push(Word { w: text.trim().to_string(), t: start, end });
            text = cur.text.clone(); start = cur.start; end = cur.end;
        } else { text.push_str(&cur.text); end = cur.end; }
    }
    words.push(Word { w: text.trim().to_string(), t: start, end });
    words.into_iter().filter(|w| !w.w.is_empty()).collect()
}

fn line_from(words: Vec<Word>, fallback_text: String, start: f64, p_end: Option<f64>, bg: bool, agent: Option<String>) -> Option<Line> {
    let text = if words.is_empty() { fallback_text.trim().to_string() } else { words.iter().map(|w| w.w.as_str()).collect::<Vec<_>>().join(" ") };
    if text.is_empty() { return None; }
    let end = words.last().map(|w| w.end).or(p_end).unwrap_or(start + 3.0).max(start + 0.2);
    let words = if words.is_empty() { spread(&text, start, end) } else { words };
    Some(Line { t: start, end, text, words, bg, agent })
}

fn parse_p(p: roxmltree::Node, offset: f64, div_agent: Option<&str>, lines: &mut Vec<Line>) {
    let first_span = p.children().filter(|c| c.has_tag_name("span")).filter_map(|c| timing(c, "begin")).map(parse_time).fold(f64::INFINITY, f64::min);
    let begin = match timing(p, "begin") { Some(b) => parse_time(b), None if first_span.is_finite() => first_span, None => return };
    let start = begin + offset;
    let agent = attr(p, "agent").or(div_agent).map(str::to_string);
    let p_bg = attr(p, "role") == Some("x-bg");
    let (mut spans, mut bgs): (Vec<Span>, Vec<Line>) = (vec![], vec![]);
    for c in p.children().filter(|c| c.has_tag_name("span")) {
        match attr(c, "role") {
            Some("x-bg") if !p_bg => {
                // voz de apoio: spans internos (ou o texto direto)
                let mut inner = vec![];
                for s in c.children().filter(|s| s.has_tag_name("span")) { if !matches!(attr(s, "role"), Some("x-translation" | "x-roman")) { word_span(s, offset, &mut inner); } }
                let b_start = timing(c, "begin").map(|b| parse_time(b) + offset).unwrap_or(start);
                let text: String = c.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect();
                if let Some(l) = line_from(merge(&inner), text, b_start, timing(c, "end").map(|e| parse_time(e) + offset), true, None) { bgs.push(l); }
            }
            Some("x-translation" | "x-roman") => {}
            _ => word_span(c, offset, &mut spans),
        }
    }
    let direct: String = p.children().filter(|c| c.is_text()).filter_map(|c| c.text()).collect();
    if let Some(l) = line_from(merge(&spans), direct, start, timing(p, "end").map(|e| parse_time(e) + offset), p_bg, agent) { lines.push(l); }
    lines.extend(bgs);
}

fn walk(n: roxmltree::Node, offset: f64, agent: Option<&str>, lines: &mut Vec<Line>) {
    for c in n.children().filter(|c| c.is_element()) {
        if c.has_tag_name("p") { parse_p(c, offset, agent, lines); }
        else if c.has_tag_name("div") { let a = attr(c, "agent").or(agent); walk(c, offset, a, lines); }
        else { walk(c, offset, agent, lines); }
    }
}

pub fn parse_ttml(ttml: &str) -> Vec<Line> {
    let Ok(doc) = roxmltree::Document::parse(ttml) else { return vec![] };
    let root = doc.root_element();
    let offset = root.descendants().find(|n| n.has_tag_name("audio")).and_then(|a| a.attribute("lyricOffset")).and_then(|o| o.parse::<f64>().ok()).unwrap_or(0.0);
    let mut lines = vec![];
    if let Some(body) = root.children().find(|c| c.has_tag_name("body")) { walk(body, offset, None, &mut lines); }
    lines
}

async fn better(t: &TrackInfo) -> Option<Lyrics> {
    let secs = t.duration_ms.map(|d| (d / 1000).to_string());
    let mut q: Vec<(&str, String)> = vec![("s", t.title.clone()), ("a", t.artist_line())];
    if let Some(d) = secs.filter(|d| d != "0") { q.push(("d", d)); }
    if let Some(a) = t.album.clone().filter(|a| !a.trim().is_empty()) { q.push(("al", a)); }
    let resp = CLIENT.get("https://lyrics-api.boidu.dev/getLyrics").query(&q).header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header("Accept", "application/json").timeout(Duration::from_secs(15)).send().await.ok()?;
    if !resp.status().is_success() { return None; }
    let ttml = resp.json::<Value>().await.ok()?.get("ttml")?.as_str()?.trim().to_string();
    let lines = parse_ttml(&ttml);
    (!lines.is_empty()).then(|| Lyrics { synced: true, provider: "BetterLyrics".into(), lines })
}

/// Ordem padrão (a do MetroFuse, mais os provedores extras do MetroMusic no fim).
pub const DEFAULT_ORDER: [&str; 12] = ["better", "lrclib", "spotify", "kugou", "apple", "musixmatch", "qq", "plus", "bini", "deezer", "ytsub", "youtube"];

/// Ordem escolhida em Configurações › Provedores de letra (sem os desligados); vale para downloads e outras buscas sem tela.
pub fn configured_order() -> Vec<String> {
    let st = crate::settings::current();
    let list = |k: &str| st.values.get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>());
    let mut order = list("lyricsOrder").unwrap_or_default();
    for d in DEFAULT_ORDER { if !order.iter().any(|o| o == d) { order.push(d.to_string()); } }
    let off = list("lyricsOff").unwrap_or_default();
    order.into_iter().filter(|p| SUPPORTED.contains(&p.as_str()) && !off.contains(p)).collect()
}

static TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)</?\s*[a-z][a-z0-9]*(?:\s[^<>]*)?/?>").unwrap());

/// Tira marcação HTML (`<font color=…>`, `<b>`, `<i>`) que algumas fontes, como as legendas do YouTube, deixam no texto.
pub(crate) fn strip_markup(s: &str) -> String {
    if !s.contains('<') { return s.to_string(); }
    TAG.replace_all(s, "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Aplica `strip_markup` a toda a letra e descarta o que ficar vazio.
fn sanitize(mut l: Lyrics) -> Lyrics {
    for line in &mut l.lines {
        line.text = strip_markup(&line.text);
        for w in &mut line.words { w.w = strip_markup(&w.w); }
        line.words.retain(|w| !w.w.is_empty());
    }
    l.lines.retain(|x| !x.text.is_empty());
    l
}

/// Tempo máximo de cada provedor (o MetroFuse usa 8 s) e de toda a busca em sequência.
const PER_PROVIDER: Duration = Duration::from_secs(8);
const TOTAL: Duration = Duration::from_secs(25);

async fn from_provider(p: &str, track: &TrackInfo) -> Option<Lyrics> {
    use crate::lyrics_more as m;
    let run = async {
        match p {
            "better" => better(track).await, "lrclib" => lrclib(track).await, "kugou" => kugou(track).await, "spotify" => spotify(track).await,
            "apple" => m::apple(track).await, "musixmatch" => m::musixmatch(track).await, "plus" => m::plus(track).await, "bini" => m::bini(track).await,
            "qq" => m::qq(track).await, "youtube" => m::youtube(track).await, "ytsub" => m::ytsub(track).await, "deezer" => m::deezer(track).await, _ => None,
        }
    };
    tokio::time::timeout(PER_PROVIDER, run).await.ok().flatten().map(sanitize).filter(|l| !l.lines.is_empty())
}

/// Depois da primeira letra sincronizada válida, quanto esperar pelos provedores que ainda não responderam
/// (uma delas pode ser palavra a palavra e vencer).
const GRACE: Duration = Duration::from_millis(3000);

/// Nota de uma letra para a escolha automática: o tipo manda (palavra a palavra > linha a linha > sem sincronia);
/// a ordem das configurações desempata; uma letra sincronizada que passa da duração da faixa é de outra versão.
pub(crate) fn score(l: &Lyrics, idx: usize, n: usize, dur_s: f64) -> i32 {
    let mut s = if has_word_timing(l) { 300 } else if l.synced { 200 } else { 50 };
    s += (n - idx.min(n)) as i32;
    if l.synced && dur_s > 0.0 {
        if l.lines.last().map_or(false, |x| x.t > dur_s + 8.0) { s -= 400; }
        if l.lines.len() < 4 { s -= 100; }
    }
    s
}

/// Letra utilizável como sincronizada: tem tempos e cabe na faixa.
fn trusted(l: &Lyrics, dur_s: f64) -> bool { l.synced && score(l, 0, 0, dur_s) >= 200 }

/// Busca a letra pelo **melhor resultado** entre os provedores de `order` (todos ao mesmo tempo): palavra a palavra
/// vence linha a linha, que vence sem sincronia; em empate vale a ordem. `None` = ninguém tem. `force` ignora o cache.
pub async fn get(track: &TrackInfo, order: &[String], force: bool) -> Res<Option<Lyrics>> {
    use futures_util::stream::{FuturesUnordered, StreamExt};
    let key = format!("{}|{}", track.key(), order.join(","));
    if !force { if let Some(hit) = CACHE.lock().get(&key) { return Ok(hit.clone()); } }
    let embedded_on = crate::settings::current().values.get("readEmbeddedLyrics").and_then(|v| v.as_bool()).unwrap_or(true);
    let mut found = if track.provider == "local" && embedded_on { crate::local::lyrics(std::path::Path::new(&track.id)) } else { None };
    if found.is_none() {
        let dur = track.duration_ms.unwrap_or(0) as f64 / 1000.0;
        let n = order.len();
        let mut pending: FuturesUnordered<_> = order.iter().enumerate().map(|(i, p)| async move { (i, from_provider(p, track).await) }).collect();
        let (start, mut first_trusted) = (std::time::Instant::now(), None::<std::time::Instant>);
        let mut done = vec![false; n];
        let mut got: Vec<(usize, Lyrics)> = vec![];
        loop {
            let left = TOTAL.saturating_sub(start.elapsed());
            let wait = first_trusted.map_or(left, |t0| left.min(GRACE.saturating_sub(t0.elapsed())));
            let Ok(Some((i, r))) = tokio::time::timeout(wait, pending.next()).await else { break };
            done[i] = true;
            if let Some(l) = r {
                if first_trusted.is_none() && trusted(&l, dur) { first_trusted = Some(std::time::Instant::now()); }
                got.push((i, l));
            }
            // nada pode superar uma palavra a palavra confiável se todos os provedores de maior prioridade já responderam
            if let Some((bi, bl)) = got.iter().max_by_key(|(i, l)| score(l, *i, n, dur)) {
                if has_word_timing(bl) && trusted(bl, dur) && done[..*bi].iter().all(|d| *d) { break; }
            }
        }
        found = got.into_iter().max_by_key(|(i, l)| score(l, *i, n, dur)).map(|(_, l)| l);
    }
    // só guarda o que foi achado: uma falha de rede não pode virar "sem letra" até reiniciar o app
    if found.is_some() { CACHE.lock().insert(key, found.clone()); } else { CACHE.lock().remove(&key); }
    Ok(found)
}

/// Uma opção da busca manual: a letra e se tem tempo por palavra de verdade (e não só distribuído pela linha).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Candidate { #[serde(flatten)] pub lyrics: Lyrics, pub word_sync: bool }

fn has_word_timing(l: &Lyrics) -> bool {
    l.synced && l.lines.iter().any(|ln| ln.words.len() > 1 && ln.words != spread(&ln.text, ln.t, ln.end))
}

/// Pergunta a todos os provedores de `order` ao mesmo tempo, com o título/artista que o usuário digitou.
/// Devolve uma letra por provedor que respondeu (na ordem pedida), para a pessoa escolher.
pub async fn search_all(base: &TrackInfo, title: &str, artist: &str, order: &[String]) -> Vec<Candidate> {
    let mut t = base.clone();
    t.title = title.trim().to_string();
    if !artist.trim().is_empty() { t.artists = artist.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(); }
    let jobs = order.iter().map(|p| { let t = t.clone(); let p = p.clone(); async move { from_provider(&p, &t).await } });
    futures_util::future::join_all(jobs).await.into_iter().flatten().map(|l| Candidate { word_sync: has_word_timing(&l), lyrics: l }).collect()
}

#[allow(dead_code)]
fn _unused(_: impl std::fmt::Display) -> String { err("") }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lrc_basico() {
        let l = parse_lrc("[ar:X]\n[00:12.50] Primeira linha\n[00:15.00]Segunda <00:15.50>linha\n[01:02.3][01:30.00] refrão\n");
        assert_eq!(l.len(), 4);
        assert_eq!(l[0].t, 12.5);
        assert_eq!(l[1].text, "Segunda linha");
        assert_eq!(l[2].t, 62.3);
        assert!(l[0].end > l[0].t && l[0].end <= 12.5 + 7.0);
        assert_eq!(l[0].words.len(), 2);
    }
    #[test]
    fn ttml_palavras_e_apoio() {
        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata"><body><div ttm:agent="v1">
          <p begin="0:10.000" end="0:14.000"><span begin="0:10.000" end="0:10.500">Hel</span><span begin="0:10.500" end="0:11.000">lo </span><span begin="0:11.000" end="0:12.000">world</span>
          <span ttm:role="x-bg"><span begin="0:12.000" end="0:13.000">(ooh)</span></span></p>
          <p begin="0:15.000" end="0:17.000"><span begin="0:15.000" end="0:16.000">Next </span><span begin="0:16.000" end="0:17.000">one</span></p></div></body></tt>"#;
        let l = parse_ttml(ttml);
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].text, "Hello world");
        assert_eq!(l[0].words.len(), 2);
        assert_eq!(l[0].words[0].w, "Hello");
        assert!(l[1].bg && l[1].text == "(ooh)");
        assert_eq!(l[2].t, 15.0);
        assert_eq!(l[0].agent.as_deref(), Some("v1"));
    }
    #[test]
    fn kugou_decodifica_e_tira_creditos() {
        use base64::Engine;
        let lrc = "[ti:X]\r\n[00:00.00]Blinding Lights - The Weeknd\r\n[00:01.75]Lyrics by：A\r\n[00:10.00]Yeah\r\n[00:14.50]I\'ve been tryna call\r\n";
        let b64 = base64::engine::general_purpose::STANDARD.encode(lrc.as_bytes());
        let lines = super::decode_kugou(&b64).unwrap();
        assert_eq!(lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), vec!["Yeah", "I've been tryna call"]);
        assert!(super::decode_kugou("não é base64!!").is_none());
    }

    #[test]
    fn detecta_tempo_por_palavra() {
        let lrc = Lyrics { synced: true, provider: "x".into(), lines: parse_lrc("[00:01.00] um dois tres\n[00:05.00] quatro") };
        assert!(!has_word_timing(&lrc));
        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="0:01.000" end="0:03.000"><span begin="0:01.000" end="0:01.200">Oi </span><span begin="0:01.200" end="0:03.000">mundo</span></p></div></body></tt>"#;
        assert!(has_word_timing(&Lyrics { synced: true, provider: "x".into(), lines: parse_ttml(ttml) }));
    }

    fn lrc(n: usize, step: f64) -> Lyrics { Lyrics { synced: true, provider: "x".into(), lines: (0..n).map(|i| Line { t: i as f64 * step, end: i as f64 * step + 2.0, text: format!("l{i}"), words: vec![], bg: false, agent: None }).collect() } }

    #[test]
    fn escolha_automatica_pela_nota() {
        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="0:01.000" end="0:03.000"><span begin="0:01.000" end="0:01.200">Oi </span><span begin="0:01.200" end="0:03.000">mundo</span></p></div></body></tt>"#;
        let mut word = Lyrics { synced: true, provider: "w".into(), lines: parse_ttml(ttml) };
        word.lines.extend(lrc(10, 10.0).lines); // 5+ linhas, cabe em 200 s
        let line = lrc(30, 6.0);
        let plain = Lyrics { synced: false, provider: "p".into(), lines: plain_lines("a\nb\nc") };
        // palavra a palavra > linha > sem sincronia, mesmo com prioridade pior
        assert!(score(&word, 5, 6, 200.0) > score(&line, 0, 6, 200.0));
        assert!(score(&line, 5, 6, 200.0) > score(&plain, 0, 6, 200.0));
        // em empate de tipo, vale a ordem
        assert!(score(&line, 0, 6, 200.0) > score(&line, 3, 6, 200.0));
        // sincronizada que passa da duração da faixa é de outra versão: perde até para a sem sincronia
        let longa = lrc(30, 10.0); // termina em 290 s numa faixa de 200 s
        assert!(score(&longa, 0, 6, 200.0) < score(&plain, 5, 6, 200.0));
        assert!(!trusted(&longa, 200.0) && trusted(&line, 200.0));
    }

    #[test]
    fn tira_marcacao_html() {
        assert_eq!(strip_markup(r##"<font color="#A0AAB4"></font><font color="485883"><b>Yu</b></font> kata <i>ne</i>"##), "Yu kata ne");
        assert_eq!(strip_markup("2 < 3 e 5 > 4"), "2 < 3 e 5 > 4");
        assert_eq!(strip_markup("sem tags"), "sem tags");
        let l = Lyrics { synced: true, provider: "x".into(), lines: parse_lrc("[00:01.00]<b>oi</b> mundo\n[00:03.00]<font color=\"#fff\"></font>") };
        let s = sanitize(l);
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.lines[0].text, "oi mundo");
    }

    #[test]
    fn limpa_titulo_e_artista() {
        assert_eq!(clean_title("Song (Official Video) [HD]"), "Song");
        assert_eq!(clean_title("Song (feat. X)"), "Song");
        assert_eq!(clean_artist("Daft Punk, Pharrell"), "Daft Punk");
    }
}
