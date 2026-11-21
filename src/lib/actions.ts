import { api, type Track } from "./ipc";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";

type Patch = (id: number, p: Partial<Track>) => void;

function apply(id: number, p: Partial<Track>, patch?: Patch) {
  if (patch) patch(id, p);
  else useUi.getState().bump(); // no local list to patch (player bar): refetch views instead
  usePlayer.getState().patchCurrent(id, p);
}

export async function toggleFavorite(t: Track, patch?: Patch) {
  const on = !t.isFavorite;
  apply(t.id, { isFavorite: on }, patch);
  try {
    await api.setFavorite(t.id, on);
  } catch {
    apply(t.id, { isFavorite: !on }, patch);
  }
}

/** Clicking the current rating clears it. */
export async function rate(t: Track, stars: number, patch?: Patch) {
  const next = t.rating === stars ? 0 : stars;
  const prev = t.rating;
  apply(t.id, { rating: next }, patch);
  try {
    await api.setRating(t.id, next);
  } catch {
    apply(t.id, { rating: prev }, patch);
  }
}
