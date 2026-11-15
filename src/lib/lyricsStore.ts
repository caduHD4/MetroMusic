import { useEffect } from "react";
import { create } from "zustand";
import { extLyrics, type Lyrics, type Track } from "./catalog";
import { engine } from "./engine";
import { getSetting } from "../store/settings";
import { LYRICS_PROVIDERS } from "./settingsSchema";
import { clearOverride, getOverride, setOverride } from "./lyricsTools";

/** Letras reais (faixas com `ext`). `version` muda quando chega uma letra nova, para os componentes recalcularem. */
interface S { version: number; loading: Record<string, boolean>; bump: () => void }
export const useLyricsStore = create<S>((set) => ({ version: 0, loading: {}, bump: () => set((s) => ({ version: s.version + 1 })) }));

const inFlight = new Set<string>();

/** Provedores na ordem das configurações, sem os desligados. `all` ignora o desligamento (busca manual). */
export function lyricsOrder(all = false): string[] {
  const saved: string[] = getSetting("lyricsOrder") ?? Object.keys(LYRICS_PROVIDERS);
  const known = [...saved.filter((p) => p in LYRICS_PROVIDERS), ...Object.keys(LYRICS_PROVIDERS).filter((p) => !saved.includes(p))];
  const off: string[] = all ? [] : getSetting("lyricsOff") ?? [];
  return known.filter((p) => !off.includes(p));
}

/** Busca a letra no backend, uma vez por faixa (`force` ignora o que já foi achado, inclusive o cache do backend).
 *  Devolve `true` se a faixa ficou com letra. Uma letra escolhida ou escrita à mão por você tem prioridade. */
export async function ensureLyrics(track: Track, force = false): Promise<boolean> {
  const id = track.id;
  const mine = force ? null : getOverride(id);
  if (mine) { extLyrics.set(id, mine); useLyricsStore.getState().bump(); return true; }
  if (!track.ext) return false;
  if ((extLyrics.has(id) && !force) || inFlight.has(id)) return !!extLyrics.get(id);
  if (force) clearOverride(id);
  inFlight.add(id);
  useLyricsStore.setState((s) => ({ loading: { ...s.loading, [id]: true } }));
  let found: Lyrics | null = null;
  try {
    found = ((await engine.lyrics(track.ext, lyricsOrder(), force)) as Lyrics | null) ?? null;
    extLyrics.set(id, found);
  } catch {
    // falha de rede não é "sem letra": se já havia uma, ela fica; senão a próxima abertura tenta de novo
    if (!extLyrics.get(id)) extLyrics.delete(id);
    found = extLyrics.get(id) ?? null;
  } finally {
    inFlight.delete(id);
    useLyricsStore.setState((s) => ({ loading: { ...s.loading, [id]: false }, version: s.version + 1 }));
  }
  return !!found;
}

/** Usa esta letra na faixa e lembra da escolha. */
export function applyLyrics(track: Track, l: Lyrics) {
  setOverride(track.id, l);
  extLyrics.set(track.id, l);
  useLyricsStore.getState().bump();
}

/** Dispara a busca e assina as atualizações; devolve se ainda está carregando. */
export function useRealLyrics(track: Track): { version: number; loading: boolean } {
  const version = useLyricsStore((s) => s.version);
  const loading = useLyricsStore((s) => !!track.ext && (s.loading[track.id] ?? !extLyrics.has(track.id)));
  useEffect(() => { void ensureLyrics(track); }, [track.id]); // eslint-disable-line react-hooks/exhaustive-deps
  return { version, loading };
}
