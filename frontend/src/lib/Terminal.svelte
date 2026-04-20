<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import "@xterm/xterm/css/xterm.css";
  import { BrowserOpenURL } from "../../wailsjs/runtime/runtime";
  import { api, themeToXterm, type Theme } from "./api";
  import { TerminalHighlighter } from "./highlight";
  import { HexFormatter } from "./hexdump";

  type Props = {
    lineEnding?: "cr" | "lf" | "crlf";
    localEcho?: boolean;
    theme?: Theme | undefined;
    fontSize?: number;
    highlight?: boolean;
    hexView?: boolean;
    timestamps?: boolean;
    // "del" (0x7f) matches VT100 / xterm convention and is what most
    // modern devices expect. "bs" (0x08) is what some older gear (some
    // Cisco IOS releases, Foundry) wants; wrong setting shows as ^H.
    backspaceKey?: "bs" | "del";
    copyOnSelect?: boolean;
    // pasteWarnMultiline prompts before sending text that crosses a
    // line. pasteSlow inserts pasteCharDelayMs between characters so
    // UART buffers on underpowered devices can drain.
    pasteWarnMultiline?: boolean;
    pasteSlow?: boolean;
    pasteCharDelayMs?: number;
    onStatus?: (msg: string) => void;
  };

  let {
    lineEnding = "crlf",
    localEcho = false,
    theme = undefined,
    fontSize = 13,
    highlight = true,
    hexView = false,
    timestamps = false,
    backspaceKey = "del",
    copyOnSelect = false,
    pasteWarnMultiline = false,
    pasteSlow = false,
    pasteCharDelayMs = 10,
    onStatus = () => {},
  }: Props = $props();

  let hostEl: HTMLDivElement;
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let unsubData: (() => void) | null = null;
  let ro: ResizeObserver | null = null;
  let highlighter: TerminalHighlighter | null = null;
  let hexFormatter: HexFormatter | null = null;
  const decoder = new TextDecoder("utf-8", { fatal: false });

  $effect(() => {
    if (highlighter && !highlight) highlighter.reset();
  });
  $effect(() => {
    if (hexFormatter && !hexView) hexFormatter.reset();
  });
  $effect(() => {
    if (highlighter && hexView) highlighter.reset();
  });

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

  // Anything longer than this arriving as a single onData callback is
  // treated as a paste, not typed input. Fast typists top out around
  // 10 chars/burst on good keyboards; 20 is comfortably above that.
  const PASTE_THRESHOLD = 20;

  function isPaste(data: string): boolean {
    return data.length >= PASTE_THRESHOLD || /[\r\n]/.test(data);
  }

  function sendByte(byte: number) {
    api.sendBytes(new Uint8Array([byte])).catch((e) => onStatus(`send failed: ${e}`));
  }

  async function sendSlow(data: string) {
    const bytes = new TextEncoder().encode(data);
    for (const b of bytes) {
      sendByte(b);
      if (localEcho && term) {
        term.write(String.fromCharCode(b));
      }
      if (pasteCharDelayMs > 0) {
        await new Promise((r) => setTimeout(r, pasteCharDelayMs));
      }
    }
  }

  function handleInput(data: string) {
    // xterm emits 0x7f for the Backspace key. Swap to 0x08 when the
    // profile asks for BS-style backspace so devices wanting that
    // don't see ^H echoed back at them.
    if (data === "\x7f" && backspaceKey === "bs") {
      data = "\x08";
    }

    // Paste handling: detect a paste-shaped input, confirm if the
    // user asked for a multi-line warning, then either slow-paste or
    // fall through to the normal path.
    if (isPaste(data)) {
      const multiline = /[\r\n]/.test(data);
      if (pasteWarnMultiline && multiline) {
        const lines = data.split(/\r\n|\r|\n/).length;
        const ok = window.confirm(
          `Send ${lines} lines to the session?\n\nFirst line: ${data.split(/\r\n|\r|\n/)[0].slice(0, 80)}`,
        );
        if (!ok) {
          onStatus("Paste cancelled");
          return;
        }
      }
      if (pasteSlow) {
        void sendSlow(data);
        return;
      }
    }

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

  $effect(() => {
    if (term && theme) {
      term.options.theme = themeToXterm(theme);
    }
  });
  $effect(() => {
    if (term && fontSize && fontSize > 0) {
      term.options.fontSize = fontSize;
      try { fit?.fit(); } catch {}
    }
  });

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
    // WebLinksAddon's default click handler is window.open, which in a
    // Wails webview either does nothing or opens the URL inside the
    // app. Route through BrowserOpenURL so clicks go to the system
    // browser like users expect.
    term.loadAddon(
      new WebLinksAddon((_event, uri) => {
        BrowserOpenURL(uri);
      }),
    );
    term.open(hostEl);
    fit.fit();
    term.focus();

    term.onData(handleInput);

    // PuTTY-style copy-on-select. xterm fires onSelectionChange during
    // the drag and once more at release; we write on every change so
    // the clipboard reflects the live selection. Empty strings are
    // ignored to avoid clobbering the clipboard when the user clicks
    // into the terminal without dragging.
    term.onSelectionChange(() => {
      if (!copyOnSelect || !term) return;
      const sel = term.getSelection();
      if (sel.length === 0) return;
      navigator.clipboard?.writeText(sel).catch(() => {});
    });

    highlighter = new TerminalHighlighter((text) => {
      if (term) term.write(text);
    });
    hexFormatter = new HexFormatter((text) => {
      if (term) term.write(text);
    });

    unsubData = api.onData((bytes) => {
      if (!term) return;
      if (hexView) {
        hexFormatter?.feed(bytes);
      } else {
        const text = decoder.decode(bytes, { stream: true });
        highlighter?.feed(text, highlight, timestamps);
      }
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
    hexFormatter?.dispose();
    hexFormatter = null;
    term?.dispose();
    term = null;
  });

  export function focus() {
    term?.focus();
  }

  export function clear() {
    term?.clear();
  }

  export function refit() {
    try {
      fit?.fit();
    } catch {}
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
