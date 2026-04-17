<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import "@xterm/xterm/css/xterm.css";
  import { api, themeToXterm, type Theme } from "./api";
  import { TerminalHighlighter } from "./highlight";

  export let lineEnding: "cr" | "lf" | "crlf" = "crlf";
  export let localEcho: boolean = false;
  export let theme: Theme | undefined = undefined;
  export let fontSize: number = 13;
  export let highlight: boolean = true;
  export let onStatus: (msg: string) => void = () => {};

  let hostEl: HTMLDivElement;
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let unsubData: (() => void) | null = null;
  let ro: ResizeObserver | null = null;
  let highlighter: TerminalHighlighter | null = null;
  const decoder = new TextDecoder("utf-8", { fatal: false });

  $: if (highlighter && !highlight) highlighter.reset();

  function eolBytes(): Uint8Array {
    switch (lineEnding) {
      case "cr":
        return new Uint8Array([0x0d]);
      case "lf":
        return new Uint8Array([0x0a]);
      case "crlf":
      default:
        return new Uint8Array([0x0d, 0x0a]);
    }
  }

  function handleInput(data: string) {
    const encoder = new TextEncoder();
    let out: Uint8Array;

    if (data === "\r") {
      out = eolBytes();
    } else {
      out = encoder.encode(data);
    }

    if (localEcho && term) {
      if (data === "\r") {
        term.write("\r\n");
      } else {
        term.write(data);
      }
    }

    api.sendBytes(out).catch((e) => onStatus(`send failed: ${e}`));
  }

  const fallbackTheme = {
    background: "#0b0b0d",
    foreground: "#e4e4e7",
    cursor: "#ffffff",
    cursorAccent: "#0b0b0d",
    selectionBackground: "#1a3a5c",
    black: "#1e1e22", red: "#ff6961", green: "#7cd992", yellow: "#f5d76e",
    blue: "#6cb6ff", magenta: "#d794ff", cyan: "#7ce0e0", white: "#d4d4d8",
    brightBlack: "#4a4a52", brightRed: "#ff8a80", brightGreen: "#a2e5b3",
    brightYellow: "#fce488", brightBlue: "#94ccff", brightMagenta: "#e5b6ff",
    brightCyan: "#a6ecec", brightWhite: "#ffffff",
  };

  $: if (term && theme) {
    term.options.theme = themeToXterm(theme);
  }
  $: if (term && fontSize && fontSize > 0) {
    term.options.fontSize = fontSize;
    try { fit?.fit(); } catch {}
  }

  onMount(() => {
    term = new Terminal({
      fontFamily: getComputedStyle(document.documentElement)
        .getPropertyValue("--font-mono")
        .trim() || "SF Mono, Menlo, monospace",
      fontSize: fontSize || 13,
      cursorBlink: true,
      cursorStyle: "block",
      allowProposedApi: true,
      scrollback: 10000,
      convertEol: false,
      theme: theme ? themeToXterm(theme) : fallbackTheme,
    });

    fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(hostEl);
    fit.fit();
    term.focus();

    term.onData(handleInput);

    highlighter = new TerminalHighlighter((text) => {
      if (term) term.write(text);
    });

    unsubData = api.onData((bytes) => {
      if (!term || !highlighter) return;
      const text = decoder.decode(bytes, { stream: true });
      highlighter.feed(text, highlight);
    });

    ro = new ResizeObserver(() => {
      try {
        fit?.fit();
      } catch {}
    });
    ro.observe(hostEl);
  });

  onDestroy(() => {
    ro?.disconnect();
    unsubData?.();
    highlighter?.dispose();
    highlighter = null;
    term?.dispose();
    term = null;
  });

  export function focus() {
    term?.focus();
  }

  export function clear() {
    term?.clear();
  }
</script>

<div class="wrap" style:background-color={theme?.background ?? "#0b0b0d"}>
  <div class="host" bind:this={hostEl}></div>
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    padding: 10px 8px 8px 12px;
    overflow: hidden;
  }

  .host {
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  :global(.xterm) {
    height: 100%;
  }

  :global(.xterm-viewport) {
    background: transparent !important;
  }
</style>
