// Central app state, exposed via Svelte context.
// We expose a getter returning a reactive object so consumers can
// access live $state values via store.X.
import { setContext, getContext } from 'svelte';

const KEY = Symbol('rbaraGui');

export function provideAppState(state) {
  setContext(KEY, state);
  return state;
}

export function useAppState() {
  return getContext(KEY);
}
