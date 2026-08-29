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

  function resetLabel(w: UsageWindow) {
    if (!w?.resets_at) return "no reset time";
    const ms = new Date(w.resets_at).getTime() - Date.now();
    if (ms <= 0) return "resetting";
    const h = Math.floor(ms / 3_600_000);
    const m = Math.round((ms % 3_600_000) / 60_000);
    return h ? `resets in ${h}h ${m}m` : `resets in ${m}m`;
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
  </div>

  <div class="body">
    {#if query.trim()}
      {#each results as { folder, item } (item.id)}
        <button
          class="item"
          onclick={() => copy(item)}
          oncontextmenu={(e) => {
            e.preventDefault();
            edit(folder, item);
          }}
        >
          <span class="label">{item.label}</span>
          <span class="preview">{folder.name}</span>
        </button>
      {:else}
        <p class="empty">No matches</p>
      {/each}
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

          {#each folder.items as item (item.id)}
            <button
              class="item"
              onclick={() => copy(item)}
              oncontextmenu={(e) => {
                e.preventDefault();
                edit(folder, item);
              }}
            >
              <span class="label">{item.label}</span>
              {#if item.text.trim() !== item.label}
                <span class="preview">{item.text}</span>
              {/if}
            </button>
          {:else}
            <p class="empty small">Copy something, then press +</p>
          {/each}
        </details>
      {/each}
    {/if}
  </div>

  <div class="footer">
    <button class="ghost" onclick={addFolder}>
      <span class="glyph">&#xE8F4;</span> New folder
    </button>
    {#if usage}
      <div class="meters">
        {@render meter("5h", usage.five_hour)}
        {@render meter("7d", usage.seven_day)}
      </div>
    {:else}
      <span class="hint" title={usageNote}>
        {usageNote || "Click to copy · Right-click to edit"}
      </span>
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

{#snippet meter(name: string, w: UsageWindow)}
  <button
    class="meter"
    title="{name}: {pct(w)}% used, {resetLabel(w)}"
    onclick={() => refreshUsage(true)}
  >
    <span class="m-name">{name}</span>
    <span class="track">
      <span
        class="fill"
        class:warn={pct(w) >= 70}
        class:hot={pct(w) >= 90}
        style="width:{pct(w)}%"
      ></span>
    </span>
    <span class="m-pct">{pct(w)}%</span>
  </button>
{/snippet}

<style>
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    margin: 12px;
    height: calc(100vh - 24px);
    background: rgba(38, 38, 38, 0.96);
    border: 1px solid var(--stroke);
    border-radius: var(--r-surface);
    box-shadow:
      0 16px 40px rgba(0, 0, 0, 0.55),
      0 2px 8px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
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
    padding: 10px 12px;
    border-bottom: 1px solid var(--stroke);
  }
  .search .glyph {
    color: var(--fg-3);
  }
  .search input {
    flex: 1;
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
  .icon:hover {
    background: var(--stroke-strong);
    color: var(--fg);
  }
  .icon.armed {
    opacity: 1;
    color: var(--danger);
    background: rgba(255, 153, 164, 0.12);
  }

  .item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 7px 10px 7px 26px;
    border-radius: var(--r-control);
    border-left: 2px solid transparent;
  }
  .item:hover {
    background: var(--layer);
    border-left-color: var(--accent);
  }
  .item:active {
    background: var(--press);
  }
  .label,
  .preview {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .preview {
    font-size: 12px;
    color: var(--fg-3);
    margin-top: 1px;
  }

  .empty {
    margin: 10px;
    color: var(--fg-3);
    font-size: 12px;
  }
  .empty.small {
    margin: 2px 0 6px 26px;
  }

  /* Footer */
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-top: 1px solid var(--stroke);
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
  .meters {
    display: flex;
    gap: 10px;
  }
  .meter {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 4px;
    border-radius: var(--r-control);
    font-size: 11px;
    color: var(--fg-3);
  }
  .meter:hover {
    background: var(--hover);
  }
  .m-name {
    font-variant-numeric: tabular-nums;
  }
  .track {
    width: 44px;
    height: 4px;
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
  .m-pct {
    min-width: 26px;
    text-align: right;
    font-variant-numeric: tabular-nums;
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

  /* Toast */
  .toast {
    position: absolute;
    bottom: 48px;
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
    background: rgba(32, 32, 32, 0.99);
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
  .spacer {
    flex: 1;
  }
</style>
