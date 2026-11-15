//! Deezer: busca e metadados pela API pública; áudio completo pela **conta** do usuário.
//!
//! Porte do caminho "conta" do `DeezerAudioProvider` (MetroFuse): o cookie `arl` abre uma sessão no
//! `gw-light`, que dá o `license_token` e o `TRACK_TOKEN` da faixa; `media.deezer.com/v1/get_url` devolve a
//! URL do CDN. O arquivo vem cifrado (BF_CBC_STRIPE): o cache decifra cada bloco ao gravar
//! (`decrypt_stripe`). Sem login ou sem direito à qualidade, `providers::resolve` procura a mesma música
//! em outro provedor e, no último caso, toca o trecho de 30 s.

use crate::http::{err, BROWSER_UA, CLIENT};
use crate::model::{now_ms, Res, StreamInfo, TrackInfo};
use blowfish::cipher::{block_padding::NoPadding, BlockDecryptMut, InnerIvInit, KeyInit};
use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

const API: &str = "https://api.deezer.com";

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(str::to_string)
}

fn to_track(v: &Value) -> Option<TrackInfo> {
    let id = v.get("id").and_then(|x| x.as_u64()).map(|x| x.to_string())?;
    let title = s(v, "title")?;
    let artist = v.get("artist").and_then(|a| s(a, "name")).unwrap_or_else(|| "Artista desconhecido".into());
    let album = v.get("album");
    Some(TrackInfo {
        provider: "deezer".into(),
        id,
        title,
        artists: vec![artist],
        album: album.and_then(|a| s(a, "title")),
        duration_ms: v.get("duration").and_then(|d| d.as_u64()).map(|d| d * 1000),
        isrc: s(v, "isrc"),
        artwork_url: album.and_then(|a| s(a, "cover_xl").or_else(|| s(a, "cover_big")).or_else(|| s(a, "cover_medium"))),
        explicit: v.get("explicit_lyrics").and_then(|x| x.as_bool()).unwrap_or(false),
        url: s(v, "link"),
        preview_url: s(v, "preview"),
        ..Default::default()
    })
}

pub async fn search(query: &str, limit: usize) -> Res<Vec<TrackInfo>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let body: Value = CLIENT
        .get(format!("{API}/search/track"))
        .query(&[("q", query), ("limit", &limit.clamp(1, 50).to_string())])
        .send().await.map_err(err)?
        .error_for_status().map_err(err)?
        .json().await.map_err(err)?;
    if let Some(e) = body.get("error") {
        return Err(format!("Deezer: {}", s(e, "message").unwrap_or_default()));
    }
    Ok(body.get("data").and_then(|d| d.as_array()).map(|a| a.iter().filter_map(to_track).collect()).unwrap_or_default())
}

/// Dados completos da faixa (inclui o ISRC, que a busca não devolve).
pub async fn track(id: &str) -> Res<TrackInfo> {
    let v: Value = CLIENT.get(format!("{API}/track/{id}")).send().await.map_err(err)?.error_for_status().map_err(err)?.json().await.map_err(err)?;
    to_track(&v).ok_or_else(|| "Deezer: faixa não encontrada".to_string())
}

/// Parada geral do Deezer (API pública, sem conta).
pub async fn chart(limit: usize) -> Res<Vec<TrackInfo>> {
    let body: Value = CLIENT.get(format!("{API}/chart/0/tracks")).query(&[("limit", limit.clamp(1, 100).to_string())])
        .send().await.map_err(err)?.error_for_status().map_err(err)?.json().await.map_err(err)?;
    Ok(body.get("data").and_then(|d| d.as_array()).map(|a| a.iter().filter_map(to_track).collect()).unwrap_or_default())
}

// ───────── conta (cookie arl)

const GW_LIGHT: &str = "https://www.deezer.com/ajax/gw-light.php";
const GET_URL: &str = "https://media.deezer.com/v1/get_url";
const SESSION_TTL_MS: u64 = 30 * 60 * 1000;
const STREAM_CACHE_MS: u64 = 45 * 60 * 1000;
const SECRET: &[u8; 16] = b"g4el58wc0zvf9na1";

static ARL: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));
static FILE: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));
static SESSION: Lazy<Mutex<Option<Session>>> = Lazy::new(|| Mutex::new(None));
static STREAMS: Lazy<Mutex<HashMap<String, StreamInfo>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct Session { api_token: String, license: String, cookie: String, expires_ms: u64, me: Me }

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Me { pub id: u64, pub name: String, pub avatar: Option<String>, pub premium: bool }

pub fn is_logged_in() -> bool { ARL.read().is_some() }

/// Cookie `arl` da sessão atual (para chamar as APIs da conta fora deste módulo, como a letra).
pub(crate) fn arl() -> Option<String> { ARL.read().clone() }

pub fn init(path: PathBuf) {
    *FILE.write() = Some(path.clone());
    if let Ok(t) = std::fs::read_to_string(&path) { let t = t.trim().to_string(); if !t.is_empty() { *ARL.write() = Some(t); } }
}

/// Aceita o valor puro do `arl` ou um cabeçalho de cookies que o contenha.
pub fn clean_arl(raw: &str) -> String {
    let t = raw.trim().trim_start_matches("Cookie:").trim();
    if let Some(v) = t.split(';').filter_map(|p| p.trim().split_once('=')).find(|(k, _)| k.trim() == "arl").map(|(_, v)| v) { return v.trim().to_string(); }
    t.split(';').next().unwrap_or("").trim().to_string()
}

pub fn set_arl(raw: &str) -> Res<()> {
    let t = clean_arl(raw);
    if t.len() < 40 || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { return Err("o arl do Deezer parece inválido (copie o valor do cookie arl em deezer.com)".into()); }
    if let Some(path) = FILE.read().clone() {
        if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
        std::fs::write(&path, &t).map_err(|e| format!("não foi possível guardar a sessão: {e}"))?;
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)); }
    }
    *ARL.write() = Some(t);
    *SESSION.lock() = None;
    STREAMS.lock().clear();
    Ok(())
}

pub fn clear() {
    *ARL.write() = None;
    *SESSION.lock() = None;
    STREAMS.lock().clear();
    if let Some(path) = FILE.read().clone() { let _ = std::fs::remove_file(path); }
}

pub fn invalidate(id: &str) { STREAMS.lock().retain(|k, _| !k.starts_with(&format!("{id}|"))); }

async fn open_session() -> Res<Session> {
    let arl = ARL.read().clone().ok_or("entre na sua conta do Deezer primeiro")?;
    let resp = CLIENT.post(GW_LIGHT).query(&[("method", "deezer.getUserData"), ("input", "3"), ("api_version", "1.0"), ("api_token", "")])
        .header("Accept", "application/json").header("User-Agent", BROWSER_UA).header("Cookie", format!("arl={arl}")).header(reqwest::header::CONTENT_LENGTH, "0").body("")
        .send().await.map_err(err)?;
    if !resp.status().is_success() { return Err(format!("Deezer HTTP {}", resp.status())); }
    let mut cookie = format!("arl={arl}");
    for c in resp.headers().get_all(reqwest::header::SET_COOKIE) {
        if let Some(pair) = c.to_str().ok().and_then(|c| c.split(';').next()) { if !pair.starts_with("arl=") { cookie.push_str("; "); cookie.push_str(pair); } }
    }
    let v: Value = resp.json().await.map_err(err)?;
    let r = v.get("results").ok_or("o Deezer não devolveu a sessão")?;
    let user = r.get("USER");
    let id = user.and_then(|u| u.get("USER_ID")).and_then(|i| i.as_u64()).unwrap_or(0);
    if id == 0 { return Err("a sessão do Deezer não é válida (o arl expirou); entre de novo".into()); }
    let opt = user.and_then(|u| u.get("OPTIONS"));
    let flag = |k: &str| opt.and_then(|o| o.get(k)).and_then(|x| x.as_bool()).unwrap_or(false);
    let name = user.and_then(|u| u.get("BLOG_NAME")).and_then(|n| n.as_str()).filter(|n| !n.is_empty()).unwrap_or("Conta Deezer").to_string();
    let avatar = user.and_then(|u| u.get("USER_PICTURE")).and_then(|p| p.as_str()).filter(|p| !p.is_empty()).map(|p| format!("https://cdn-images.dzcdn.net/images/user/{p}/250x250-000000-80-0-0.jpg"));
    Ok(Session {
        api_token: r.get("checkForm").and_then(|c| c.as_str()).ok_or("o Deezer não devolveu o token da sessão")?.to_string(),
        license: opt.and_then(|o| o.get("license_token")).and_then(|l| l.as_str()).unwrap_or("").to_string(),
        cookie, expires_ms: now_ms() + SESSION_TTL_MS,
        me: Me { id, name, avatar, premium: flag("web_hq") || flag("web_lossless") || flag("mobile_hq") || flag("mobile_lossless") },
    })
}

async fn session(force: bool) -> Res<Session> {
    if !force { if let Some(s) = SESSION.lock().clone().filter(|s| s.expires_ms > now_ms()) { return Ok(s); } }
    let s = open_session().await?;
    *SESSION.lock() = Some(s.clone());
    Ok(s)
}

/// Valida a sessão atual e devolve o perfil.
pub async fn me() -> Res<Me> { Ok(session(true).await?.me) }

async fn track_token(s: &Session, id: &str) -> Res<String> {
    let sng: Value = id.parse::<u64>().map(Value::from).unwrap_or_else(|_| Value::from(id));
    let v: Value = CLIENT.post(GW_LIGHT).query(&[("method", "song.getListData"), ("input", "3"), ("api_version", "1.0"), ("api_token", s.api_token.as_str())])
        .header("Accept", "application/json").header("User-Agent", BROWSER_UA).header("Cookie", &s.cookie).json(&json!({ "sng_ids": [sng] }))
        .send().await.map_err(err)?.error_for_status().map_err(err)?.json().await.map_err(err)?;
    v.pointer("/results/data/0/TRACK_TOKEN").and_then(|t| t.as_str()).map(str::to_string).ok_or_else(|| "o Deezer não deu o token desta faixa".to_string())
}

/// Formatos pedidos, do preferido ao último recurso (`deezerQuality`: `flac` · `320` · `128`).
fn quality_order() -> Vec<&'static str> {
    let q = crate::settings::current().values.get("deezerQuality").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "flac".into());
    match q.as_str() { "128" => vec!["MP3_128"], "320" => vec!["MP3_320", "MP3_128"], _ => vec!["FLAC", "MP3_320", "MP3_128"] }
}

fn format_info(f: &str) -> (&'static str, &'static str, &'static str, u32) {
    match f { "FLAC" => ("audio/flac", "flac", "flac", 1_411_000), "MP3_320" => ("audio/mpeg", "mp3", "mp3", 320_000), _ => ("audio/mpeg", "mp3", "mp3", 128_000) }
}

/// Stream completo pela conta. O arquivo é cifrado: `StreamInfo::cipher` leva o id para o cache decifrar.
pub async fn resolve(track: &TrackInfo) -> Res<StreamInfo> {
    let qualities = quality_order();
    let key = format!("{}|{}", track.id, qualities[0]);
    if let Some(s) = STREAMS.lock().get(&key).filter(|s| s.expires_at_ms > now_ms() + 30_000).cloned() { return Ok(s); }
    let mut sess = session(false).await?;
    let token = match track_token(&sess, &track.id).await {
        Ok(t) => t,
        Err(_) => { sess = session(true).await?; track_token(&sess, &track.id).await? } // sessão caída: reabre uma vez
    };
    let formats: Vec<Value> = qualities.iter().map(|f| json!({ "cipher": "BF_CBC_STRIPE", "format": f })).collect();
    let v: Value = CLIENT.post(GET_URL).header("Accept", "application/json").header("User-Agent", BROWSER_UA).header("Cookie", &sess.cookie)
        .json(&json!({ "track_tokens": [token], "license_token": sess.license, "media": [{ "type": "FULL", "formats": formats }] }))
        .send().await.map_err(err)?.error_for_status().map_err(err)?.json().await.map_err(err)?;
    let item = v.pointer("/data/0").ok_or("o Deezer não respondeu ao pedido de áudio")?;
    let medias = item.get("media").and_then(|m| m.as_array()).cloned().unwrap_or_default();
    let Some(media) = qualities.iter().find_map(|q| medias.iter().find(|m| m.get("format").and_then(|f| f.as_str()) == Some(q))).or_else(|| medias.first()) else {
        let why = item.pointer("/errors/0/message").and_then(|m| m.as_str()).unwrap_or("sem direito a esta faixa");
        return Err(format!("Deezer: {why}"));
    };
    let sources = media.get("sources").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let url = sources.get(1).or_else(|| sources.first()).and_then(|s| s.get("url")).and_then(|u| u.as_str()).ok_or("o Deezer não devolveu a URL do áudio")?.to_string();
    let cipher_type = media.pointer("/cipher/type").and_then(|c| c.as_str()).unwrap_or("NONE");
    let cipher = match cipher_type { "BF_CBC_STRIPE" => Some(track.id.clone()), "NONE" => None, other => return Err(format!("Deezer: cifra não suportada ({other})")) };
    let format = media.get("format").and_then(|f| f.as_str()).unwrap_or("MP3_128");
    let (mime, codec, ext, bitrate) = format_info(format);
    let expires = media.get("exp").and_then(|e| e.as_u64()).map(|e| (e * 1000).saturating_sub(30_000)).filter(|e| *e > now_ms() + 60_000).unwrap_or(now_ms() + STREAM_CACHE_MS);
    let info = StreamInfo {
        url, headers: vec![("User-Agent".into(), BROWSER_UA.into()), ("Accept-Encoding".into(), "identity".into())],
        mime: mime.into(), codec: codec.into(), bitrate,
        content_length: media.get("filesize").and_then(|f| f.as_u64()).filter(|f| *f > 0),
        ext: ext.into(), expires_at_ms: expires, source: "deezer".into(), source_id: track.id.clone(), segments: vec![], loudness_lufs: None, cipher,
    };
    STREAMS.lock().insert(key, info.clone());
    Ok(info)
}

// ───────── decifragem

/// Decifra em lugar o trecho `data`, que começa no byte `start` (múltiplo de 2048) do arquivo.
/// BF_CBC_STRIPE: em blocos de 2048 bytes, só os de índice múltiplo de 3 (e completos) estão cifrados,
/// em Blowfish-CBC com IV `00 01 … 07`; a chave deriva do MD5 hexadecimal do id da faixa.
pub fn decrypt_stripe(track_id: &str, start: u64, data: &mut [u8]) -> Res<()> {
    const BLOCK: usize = 2048;
    if start % BLOCK as u64 != 0 { return Err("Deezer: trecho desalinhado para decifrar".into()); }
    let hex: Vec<u8> = Md5::digest(track_id.as_bytes()).iter().flat_map(|b| format!("{b:02x}").into_bytes()).collect();
    let key: Vec<u8> = (0..16).map(|i| hex[i] ^ hex[i + 16] ^ SECRET[i]).collect();
    let bf = blowfish::Blowfish::new_from_slice(&key).map_err(|e| format!("Deezer: chave inválida: {e}"))?;
    let first = start / BLOCK as u64;
    for (i, block) in data.chunks_exact_mut(BLOCK).enumerate() {
        if (first + i as u64) % 3 != 0 { continue; }
        let dec = cbc::Decryptor::<blowfish::Blowfish>::inner_iv_slice_init(bf.clone(), &[0, 1, 2, 3, 4, 5, 6, 7]).map_err(|e| format!("Deezer: IV inválido: {e}"))?;
        dec.decrypt_padded_mut::<NoPadding>(block).map_err(|e| format!("Deezer: bloco inválido: {e}"))?;
    }
    Ok(())
}
