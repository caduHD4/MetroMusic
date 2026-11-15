//! Leitura e escrita de arquivos escolhidos pelo usuário nos diálogos (backup, listas M3U/CSV).
//! Os caminhos vêm do seletor nativo; aqui só se valida o tamanho e se grava de forma atômica.

use crate::model::Res;
use std::path::Path;

/// Limite de leitura: backup e listas são texto pequeno; recusar o resto evita travar com um arquivo qualquer.
pub const MAX_READ: u64 = 64 * 1024 * 1024;

pub fn read_text(path: &Path) -> Res<String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("não consegui abrir o arquivo: {e}"))?;
    if !meta.is_file() { return Err("isso não é um arquivo".into()); }
    if meta.len() > MAX_READ { return Err("o arquivo é grande demais".into()); }
    let bytes = std::fs::read(path).map_err(|e| format!("não consegui ler o arquivo: {e}"))?;
    String::from_utf8(bytes).map_err(|_| "o arquivo não é um texto UTF-8".to_string())
}

pub fn write_text(path: &Path, text: &str) -> Res<()> {
    if path.as_os_str().is_empty() { return Err("caminho vazio".into()); }
    if path.is_dir() { return Err("o destino é uma pasta".into()); }
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, text).map_err(|e| format!("não consegui gravar o arquivo: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| { let _ = std::fs::remove_file(&tmp); format!("não consegui concluir a gravação: {e}") })
}

#[tauri::command]
pub fn file_read_text(path: String) -> Res<String> { read_text(Path::new(&path)) }

/// Grava bytes (a imagem da letra) no caminho escolhido no seletor, vindos em base64.
pub fn write_base64(path: &Path, b64: &str) -> Res<()> {
    use base64::Engine;
    if path.as_os_str().is_empty() { return Err("caminho vazio".into()); }
    if path.is_dir() { return Err("o destino é uma pasta".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| "dados da imagem inválidos".to_string())?;
    if bytes.len() as u64 > MAX_READ { return Err("o arquivo é grande demais".into()); }
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).map_err(|e| format!("não consegui gravar o arquivo: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| { let _ = std::fs::remove_file(&tmp); format!("não consegui concluir a gravação: {e}") })
}

#[tauri::command]
pub fn file_write_base64(path: String, data: String) -> Res<()> { write_base64(Path::new(&path), &data) }

#[tauri::command]
pub fn file_write_text(path: String, text: String) -> Res<()> { write_text(Path::new(&path), &text) }

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("mm-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn grava_base64() {
        let d = dir("b64");
        let f = d.join("a.png");
        write_base64(&f, "iVBORw==").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), vec![0x89, b'P', b'N', b'G']);
        assert!(write_base64(&f, "***").is_err());
    }

    #[test]
    fn grava_le_e_substitui() {
        let d = dir("rw");
        let f = d.join("a.backup");
        write_text(&f, "{\"a\":1}").unwrap();
        assert_eq!(read_text(&f).unwrap(), "{\"a\":1}");
        write_text(&f, "novo ✓").unwrap();
        assert_eq!(read_text(&f).unwrap(), "novo ✓");
        assert!(!d.join("a.part").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn erros_claros() {
        let d = dir("err");
        assert!(read_text(&d.join("nao-existe")).unwrap_err().contains("abrir"));
        assert!(read_text(&d).unwrap_err().contains("não é um arquivo"));
        assert!(write_text(&d, "x").is_err());
        assert!(write_text(Path::new(""), "x").is_err());
        std::fs::write(d.join("bin"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(read_text(&d.join("bin")).unwrap_err().contains("UTF-8"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
