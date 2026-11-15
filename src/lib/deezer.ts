// Sessão do Deezer (cookie arl): o áudio completo vem da conta; sem ela a faixa cai para outro provedor.
import { create } from "zustand";
import { engine, type DzAccountInfo } from "./engine";
import { isTauri, onEvent } from "./native";
import { useLib } from "../store/library";
import { toast, useUI } from "../store/ui";

interface S { info: DzAccountInfo | null; status: "idle" | "waiting" | "checking"; error: string | null }
const KEY = "metromusic-dz-account-v1";
const saved = (): DzAccountInfo | null => { try { return JSON.parse(localStorage.getItem(KEY) ?? "null"); } catch { return null; } };
export const useDz = create<S>(() => ({ info: saved(), status: "idle", error: null }));

function setInfo(info: DzAccountInfo | null) {
  try { info ? localStorage.setItem(KEY, JSON.stringify(info)) : localStorage.removeItem(KEY); } catch { /* sem armazenamento */ }
  useDz.setState({ info, status: "idle", error: null });
  useLib.getState().setAccount("deezer", info ? info.name : null);
}

function onLoggedIn(info: DzAccountInfo) {
  setInfo(info);
  useUI.getState().closeDialog();
  toast(info.premium ? `Conectado ao Deezer como ${info.name}` : `Conectado ao Deezer como ${info.name} (conta gratuita: MP3 128 kbps)`);
}

export function loginDeezerWeb() { useDz.setState({ status: "waiting", error: null }); engine.deezerLoginStart().catch((e) => useDz.setState({ status: "idle", error: String(e) })); }
export function cancelDeezerLogin() { engine.deezerLoginCancel(); useDz.setState({ status: "idle" }); }
export async function loginDeezerToken(token: string) {
  useDz.setState({ status: "checking", error: null });
  try {
    const info = await engine.deezerLoginToken(token);
    if (info) onLoggedIn(info); else useDz.setState({ status: "idle" });
  } catch (e) { useDz.setState({ status: "idle", error: String(e) }); }
}
export async function logoutDeezer() { await engine.deezerLogout(); setInfo(null); toast("Você saiu do Deezer"); }

export async function initDeezer() {
  if (!isTauri) return;
  await onEvent<{ ok: boolean; cancelled: boolean; error?: string | null; account?: DzAccountInfo | null }>("dz:login", (e) => {
    if (e.ok && e.account) onLoggedIn(e.account);
    else useDz.setState({ status: "idle", error: e.cancelled ? null : (e.error ?? "Não foi possível entrar") });
  });
  try {
    const info = (await engine.deezerAccount()) ?? null;
    if (info) setInfo(info); else if (useDz.getState().info) setInfo(null);
  } catch { /* sem rede: mantém o guardado */ }
  if (!useDz.getState().info && useLib.getState().accounts.deezer) useLib.getState().setAccount("deezer", null);
}
