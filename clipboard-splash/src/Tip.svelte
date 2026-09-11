<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { LogicalSize } from "@tauri-apps/api/dpi";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  // The Claude Mode label: one line, white, system font, no chrome of any kind.
  // Rust owns where it sits and how long it lives; this end only draws it and
  // reports back how big it turned out.
  let text = $state("");
  let box = $state<HTMLElement | undefined>();

  void listen<string>("tip", (event) => (text = event.payload));

  $effect(() => {
    text;
    if (!box) return;
    const rect = box.getBoundingClientRect();
    void getCurrentWindow().setSize(
      new LogicalSize(Math.ceil(rect.width), Math.ceil(rect.height)),
    );
  });
</script>

<span class="tip" bind:this={box}>{text}</span>

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
</style>
