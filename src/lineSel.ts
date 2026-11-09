// The diff view registers here so the "S" shortcut can act on selected lines first.
export const lineSel: { handler: (() => boolean) | null } = { handler: null }
