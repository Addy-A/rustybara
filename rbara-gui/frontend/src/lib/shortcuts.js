// Direct-command keys handled by the hardcoded switch in App.svelte's handleKey
// (o=overwrite, f=add files, v=view, a/n/i=scoping, ?=help, :=command bar). An
// action shortcut bound to one of these would shadow the command — the rebindable
// dispatch runs before the switch — so they can never be assigned to an action.
export const RESERVED_KEYS = ['o', 'f', 'v', 'a', 'n', 'i', '?', ':']
