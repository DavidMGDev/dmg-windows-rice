<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { LogicalSize } from "@tauri-apps/api/dpi";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  /// Rust owns where this sits and how long it lives; the webview draws it and
  /// reports back how wide it came out, so the box hugs the text.
  type View = { text: string; small: boolean; dot: boolean };

  let view = $state<View>({ text: "", small: false, dot: false });
  let box = $state<HTMLElement | undefined>();

  void listen<View>("tip", (event) => (view = event.payload));

  $effect(() => {
    view;
    if (!box) return;
    const rect = box.getBoundingClientRect();
    void getCurrentWindow().setSize(
      new LogicalSize(Math.ceil(rect.width), Math.ceil(rect.height)),
    );
  });
</script>

<span class="tip" class:small={view.small} bind:this={box}>{#if view.dot}<i class="dot"></i>{/if}{view.text}</span>

<style>
  .tip {
    display: inline-block;
    padding: 2px 5px;
    background: #fff;
    color: #000;
    font-family: "Segoe UI", system-ui, sans-serif;
    font-size: 12px;
    line-height: 15px;
    white-space: nowrap;
  }

  /* An answer to something you typed: smaller, so twice as much of it fits. */
  .tip.small {
    font-size: 11px;
    line-height: 14px;
  }

  /* The rest of the answer is on the clipboard. Drawn rather than written, so
     it cannot be read as a character Claude chose to put there. */
  .dot {
    display: inline-block;
    width: 4px;
    height: 4px;
    margin-right: 5px;
    vertical-align: middle;
    border-radius: 50%;
    background: #c4c4c4;
  }
</style>
