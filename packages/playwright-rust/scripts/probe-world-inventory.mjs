// Diagnostic metadata only: never retain an event payload, origin or page value.
export class WorldInventory {
  #frame;
  #contexts = new Map();
  #overflow = false;

  constructor(frame) { this.#frame = frame; }

  created({ context }) {
    if (context?.auxData?.frameId !== this.#frame) return;
    const { id, name, auxData } = context;
    if (!Number.isSafeInteger(id) || typeof name !== 'string' || name.length > 128
        || (!this.#contexts.has(id) && this.#contexts.size >= 512)) {
      this.#overflow = true;
      return;
    }
    const kind = auxData.isDefault === true ? 'main'
      : name.startsWith('__playwright_rust_utility_') ? 'playwrightRust'
      : name.startsWith('__playwright_utility_world_') ? 'playwright' : 'other';
    this.#contexts.set(id, { kind, name: kind === 'playwrightRust' ? name : '' });
  }

  reset() { this.#contexts.clear(); }

  snapshot() {
    const counts = { main: 0, playwrightRust: 0, playwright: 0, other: 0 };
    const names = new Set();
    const mainContextIds = [];
    for (const [id, { kind, name }] of this.#contexts) {
      counts[kind]++;
      if (kind === 'playwrightRust') names.add(name);
      if (kind === 'main') mainContextIds.push(id);
    }
    return { counts, playwrightRustNames: [...names].sort(), mainContextIds: mainContextIds.sort((a, b) => a - b), overflow: this.#overflow };
  }
}
