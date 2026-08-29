<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { onMount, tick } from "svelte";

  type Item = { id: string; label: string; text: string };
  type Folder = { id: string; name: string; open: boolean; items: Item[] };
  type UsageWindow = { utilization: number | null; resets_at: string | null } | null;
  type Usage = { five_hour: UsageWindow; seven_day: UsageWindow };

  const win = getCurrentWindow();
  const uid = () => Math.random().toString(36).slice(2, 10);
  const firstLine = (s: string) =>
    (s.split("\n")[0] ?? "").trim().slice(0, 48) || "Untitled";

  let folders = $state<Folder[]>([]);
  let loaded = $state(false);
  let query = $state("");
  let toast = $state("");
  let editing = $state<{ folderId: string; item: Item; isNew: boolean } | null>(null);
  let renaming = $state<string | null>(null);
  let armedFolder = $state<string | null>(null);

  let searchEl = $state<HTMLInputElement | undefined>();
  let labelEl = $state<HTMLInputElement | undefined>();

  let usage = $state<Usage | null>(null);
  let usageNote = $state("");
  let usageAt = 0;

  /** Claude Code caches this for 5 minutes; no reason for us to poll harder. */
  async function refreshUsage(force = false) {
    if (!force && Date.now() - usageAt < 300_000) return;
    usageAt = Date.now();
    try {
      usage = await invoke<Usage>("fetch_usage");
      usageNote = "";
    } catch (e) {
      usage = null;
      usageNote = String(e);
    }
  }

  const pct = (w: UsageWindow) =>
    Math.min(100, Math.max(0, Math.round(w?.utilization ?? 0)));

  /** Compact enough to sit under a 3px bar: "2h 14m", "5d 3h", "18m". */
  function resetLabel(w: UsageWindow) {
    if (!w?.resets_at) return "—";
    const ms = new Date(w.resets_at).getTime() - Date.now();
    if (ms <= 0) return "now";
    const mins = Math.floor(ms / 60_000);
    const hours = Math.floor(mins / 60);
    const days = Math.floor(hours / 24);
    if (days) return `${days}d ${hours % 24}h`;
    if (hours) return `${hours}h ${mins % 60}m`;
    return `${mins}m`;
  }

  const results = $derived.by(() => {
    const t = query.trim().toLowerCase();
    if (!t) return [];
    const out: { folder: Folder; item: Item }[] = [];
    for (const f of folders)
      for (const item of f.items)
        if (
          item.label.toLowerCase().includes(t) ||
          item.text.toLowerCase().includes(t)
        )
          out.push({ folder: f, item });
    return out;
  });

  onMount(() => {
    // The overlay is never unmounted, so the listener needs no teardown.
    void listen("shown", resetView);
    void hydrate();
    void refreshUsage();
  });

  async function hydrate() {
    const raw = await invoke<string>("load_clips");
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed?.folders)) folders = parsed.folders;
    } catch {
      /* corrupt store: fall through to the seed below */
    }
    if (!raw)
      folders = [
        { id: uid(), name: "Paths", open: true, items: [] },
        { id: uid(), name: "Prompts", open: true, items: [] },
      ];
    loaded = true;
  }

  $effect(() => {
    const data = JSON.stringify({ folders });
    if (!loaded) return;
    invoke("save_clips", { data });
  });

  async function resetView() {
    query = "";
    toast = "";
    editing = null;
    renaming = null;
    armedFolder = null;
    await tick();
    searchEl?.focus();
    void refreshUsage();
  }

  async function copy(item: Item) {
    await writeText(item.text);
    toast = "Copied";
    setTimeout(() => win.hide(), 320);
  }

  async function addFrom(folder: Folder) {
    const text = (await readText()) ?? "";
    folder.open = true;
    editing = {
      folderId: folder.id,
      item: { id: uid(), label: firstLine(text), text },
      isNew: true,
    };
    await tick();
    labelEl?.focus();
    labelEl?.select();
  }

  function edit(folder: Folder, item: Item) {
    editing = { folderId: folder.id, item: { ...item }, isNew: false };
  }

  function saveEdit() {
    if (!editing) return;
    const target = editing;
    const folder = folders.find((f) => f.id === target.folderId);
    if (folder) {
      const item = {
        ...target.item,
        label: target.item.label.trim() || firstLine(target.item.text),
      };
      const i = folder.items.findIndex((x) => x.id === item.id);
      if (i >= 0) folder.items[i] = item;
      else folder.items.push(item);
    }
    editing = null;
  }

  function deleteItem() {
    if (!editing) return;
    const target = editing;
    const folder = folders.find((f) => f.id === target.folderId);
    if (folder) folder.items = folder.items.filter((x) => x.id !== target.item.id);
    editing = null;
  }

  function addFolder() {
    const folder: Folder = { id: uid(), name: "New folder", open: true, items: [] };
    folders.push(folder);
    renaming = folder.id;
  }

  function removeFolder(folder: Folder) {
    // Two-step: first click arms, second confirms. Cheaper than a dialog.
    if (folder.items.length && armedFolder !== folder.id) {
      armedFolder = folder.id;
      return;
    }
    folders = folders.filter((f) => f.id !== folder.id);
    armedFolder = null;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (editing) editing = null;
      else if (renaming) renaming = null;
      else win.hide();
    } else if (e.key === "Enter" && !editing && results.length) {
      copy(results[0].item);
    }
  }

  /** Stops a control inside <summary> from toggling the folder. */
  function isolate(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={(e) => e.preventDefault()} />

<div class="panel">
  <div class="search">
    <span class="glyph">&#xE721;</span>
    <input
      bind:this={searchEl}
      bind:value={query}
      placeholder="Search clips"
      spellcheck="false"
    />
    <button class="icon pinned" title="New folder" onclick={addFolder}>&#xE8F4;</button>
  </div>

  <div class="body">
    {#if query.trim()}
      <div class="grid">
        {#each results as { folder, item } (item.id)}
          {@render tile(folder, item, folder.name)}
        {:else}
          <p class="empty">No matches</p>
        {/each}
      </div>
    {:else}
      {#each folders as folder (folder.id)}
        <details bind:open={folder.open}>
          <summary>
            <span class="chev">&#xE76C;</span>
            {#if renaming === folder.id}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="rename"
                autofocus
                bind:value={folder.name}
                onclick={isolate}
                onblur={() => (renaming = null)}
                onkeydown={(e) => e.key === "Enter" && (renaming = null)}
              />
            {:else}
              <button
                class="name"
                title="Double-click to rename"
                ondblclick={(e) => {
                  isolate(e);
                  renaming = folder.id;
                }}
              >
                {folder.name}
              </button>
            {/if}
            <span class="count">{folder.items.length}</span>
            <button
              class="icon"
              title="Save clipboard here"
              onclick={(e) => {
                isolate(e);
                addFrom(folder);
              }}>&#xE710;</button
            >
            <button
              class="icon"
              class:armed={armedFolder === folder.id}
              title={armedFolder === folder.id
                ? "Click again to delete"
                : "Delete folder"}
              onclick={(e) => {
                isolate(e);
                removeFolder(folder);
              }}>&#xE74D;</button
            >
          </summary>

          <div class="grid">
            {#each folder.items as item (item.id)}
              {@render tile(folder, item, "")}
            {:else}
              <p class="empty">Copy something, then press +</p>
            {/each}
          </div>
        </details>
      {/each}
    {/if}
  </div>

  <div class="footer">
    <span class="brand" title="Claude plan usage">
      <svg class="mark" viewBox="0 0 32 32" aria-hidden="true">
        {#each [0, 30, 60, 90, 120, 150] as angle}
          <rect x="14.9" y="3" width="2.2" height="26" rx="1.1" transform="rotate({angle} 16 16)" />
        {/each}
      </svg>
    </span>
    {#if usage}
      {@render meter("5h", usage.five_hour)}
      {@render meter("7d", usage.seven_day)}
    {:else}
      <span class="hint" title={usageNote}>{usageNote || "Usage unavailable"}</span>
    {/if}
  </div>

  {#if toast}
    <div class="toast">{toast}</div>
  {/if}

  {#if editing}
    <div class="editor">
      <input
        bind:this={labelEl}
        class="field"
        bind:value={editing.item.label}
        placeholder="Name"
        spellcheck="false"
      />
      <textarea
        class="field grow"
        bind:value={editing.item.text}
        placeholder="Content"
        spellcheck="false"
      ></textarea>
      <div class="row">
        {#if !editing.isNew}
          <button class="ghost danger" onclick={deleteItem}>
            <span class="glyph">&#xE74D;</span> Delete
          </button>
        {/if}
        <span class="spacer"></span>
        <button class="ghost" onclick={() => (editing = null)}>Cancel</button>
        <button class="accent" onclick={saveEdit}>Save</button>
      </div>
    </div>
  {/if}
</div>

{#snippet tile(folder: Folder, item: Item, badge: string)}
  <button
    class="tile"
    onclick={() => copy(item)}
    oncontextmenu={(e) => {
      e.preventDefault();
      edit(folder, item);
    }}
  >
    <span class="t-label">{item.label}</span>
    <span class="t-preview">{item.text}</span>
    {#if badge}<span class="t-badge">{badge}</span>{/if}
  </button>
{/snippet}

{#snippet meter(name: string, w: UsageWindow)}
  <button
    class="meter"
    title="{name} window, {pct(w)}% used"
    onclick={() => refreshUsage(true)}
  >
    <span class="m-top">
      <span class="m-name">{name}</span>
      <span class="m-pct">{pct(w)}%</span>
    </span>
    <span class="track">
      <span
        class="fill"
        class:warn={pct(w) >= 70}
        class:hot={pct(w) >= 90}
        style="width:{pct(w)}%"
      ></span>
    </span>
    <span class="m-reset">{resetLabel(w)}</span>
  </button>
{/snippet}

<style>
  /* The window itself is opaque and DWM rounds and shadows it, so there is no
     CSS shadow to get clipped by the window rect. */
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #262626;
    overflow: hidden;
  }

  .glyph,
  .chev,
  .icon {
    font-family: "Segoe Fluent Icons", "Segoe MDL2 Assets";
    font-size: 12px;
    line-height: 1;
  }

  /* Search */
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 10px 10px 12px;
    border-bottom: 1px solid var(--stroke);
  }
  .search .glyph {
    color: var(--fg-3);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    outline: none;
    padding: 2px 0;
    user-select: text;
  }
  .search input::placeholder {
    color: var(--fg-3);
  }

  /* List */
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }

  summary {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: var(--r-control);
    font-size: 12px;
    color: var(--fg-2);
    list-style: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary:hover {
    background: var(--hover);
  }
  .chev {
    color: var(--fg-3);
    transition: transform 120ms ease;
  }
  details[open] .chev {
    transform: rotate(90deg);
  }
  .name {
    padding: 0;
    font-weight: 600;
  }
  .count {
    margin-left: auto;
    color: var(--fg-3);
    font-variant-numeric: tabular-nums;
  }
  .rename {
    flex: 1;
    background: var(--press);
    border: 1px solid var(--accent);
    border-radius: var(--r-control);
    outline: none;
    padding: 1px 4px;
    user-select: text;
  }

  .icon {
    width: 22px;
    height: 22px;
    border-radius: var(--r-control);
    color: var(--fg-3);
    opacity: 0;
  }
  summary:hover .icon {
    opacity: 1;
  }
  .icon.pinned {
    opacity: 1;
  }
  .icon:hover {
    background: var(--stroke-strong);
    color: var(--fg);
  }
  .icon.armed {
    opacity: 1;
    color: var(--danger);
    background: rgba(255, 153, 164, 0.12);
  }

  /* Gallery */
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    padding: 4px 4px 10px;
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-height: 60px;
    padding: 8px 10px;
    text-align: left;
    background: var(--layer);
    border: 1px solid var(--stroke);
    border-radius: 6px;
    transition:
      background 120ms ease,
      border-color 120ms ease;
  }
  .tile:hover {
    background: var(--hover);
    border-color: var(--stroke-strong);
  }
  .tile:active {
    background: var(--press);
  }
  .t-label {
    font-size: 12.5px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .t-preview {
    font-size: 11px;
    line-height: 1.35;
    color: var(--fg-3);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-all;
  }
  .t-badge {
    align-self: flex-start;
    margin-top: 2px;
    padding: 1px 6px;
    font-size: 10px;
    color: var(--fg-3);
    background: var(--press);
    border-radius: 999px;
  }

  .empty {
    grid-column: 1 / -1;
    margin: 6px 4px 2px;
    color: var(--fg-3);
    font-size: 12px;
  }

  /* Footer */
  .footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-top: 1px solid var(--stroke);
  }
  .brand {
    display: flex;
    flex: none;
  }
  .mark {
    width: 17px;
    height: 17px;
    fill: #d97757;
  }
  .hint {
    flex: 1;
    min-width: 0;
    text-align: right;
    font-size: 11px;
    color: var(--fg-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Plan usage */
  .meter {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 3px 7px;
    border-radius: var(--r-control);
  }
  .meter:hover {
    background: var(--hover);
  }
  .m-top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    font-size: 10.5px;
    color: var(--fg-3);
  }
  .m-name {
    font-weight: 600;
    color: var(--fg-2);
  }
  .m-pct {
    font-variant-numeric: tabular-nums;
  }
  .track {
    width: 100%;
    height: 3px;
    border-radius: 2px;
    background: var(--stroke-strong);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 200ms ease;
  }
  .fill.warn {
    background: #fce100;
  }
  .fill.hot {
    background: var(--danger);
  }
  .m-reset {
    font-size: 10px;
    color: var(--fg-3);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* Toast */
  .toast {
    position: absolute;
    bottom: 56px;
    left: 50%;
    transform: translateX(-50%);
    padding: 5px 14px;
    font-size: 12px;
    background: rgba(20, 20, 20, 0.94);
    border: 1px solid var(--stroke-strong);
    border-radius: 999px;
  }

  /* Editor */
  .editor {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    background: #202020;
  }
  .field {
    background: var(--layer);
    border: 1px solid var(--stroke);
    border-bottom-color: var(--stroke-strong);
    border-radius: var(--r-control);
    outline: none;
    padding: 7px 10px;
    user-select: text;
    resize: none;
  }
  .field:focus {
    border-bottom: 2px solid var(--accent);
    padding-bottom: 6px;
  }
  .grow {
    flex: 1;
    font-family: "Cascadia Mono", "Consolas", monospace;
    font-size: 12px;
    line-height: 1.5;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ghost,
  .accent {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    font-size: 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--stroke);
    background: var(--layer);
  }
  .ghost:hover {
    background: var(--hover);
  }
  .ghost.danger {
    color: var(--danger);
  }
  .accent {
    background: var(--accent);
    border-color: transparent;
    color: #003e5c;
    font-weight: 600;
  }
  .accent:hover {
    opacity: 0.9;
  }
  .spacer {
    flex: 1;
  }
</style>
