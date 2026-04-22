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
    screenReaderMode?: boolean;
    onStatus?: (msg: string) => void;
    // Called when a multi-line paste arrives and pasteWarnMultiline is
    // enabled. Host app is expected to render a modal (window.confirm
    // is a no-op in WKWebView since Wails v2 doesn't wire WKUIDelegate
    // through) and resolve true/false with the user's choice. If not
    // provided, multi-line pastes go through without confirmation.
    onPasteConfirm?: (data: string) => Promise<boolean> | boolean;
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
    screenReaderMode = false,
    onStatus = () => {},
    onPasteConfirm,
  }: Props = $props();

  let hostEl: HTMLDivElement;
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let unsubData: (() => void) | null = null;
  let ro: ResizeObserver | null = null;
  let highlighter: TerminalHighlighter | null = null;
  let hexFormatter: HexFormatter | null = null;
  const decoder = new TextDecoder("utf-8", { fatal: false });

  // Svelte 5's runtime dependency tracker only registers a prop
  // read if it actually happens during the effect's execution. If
  // the condition short-circuits on a plain (non-reactive) variable
  // like highlighter/hexFormatter, the prop is never read on the
  // first run, never tracked, and subsequent prop changes don't
  // fire the effect. Read the reactive props unconditionally first.
  $effect(() => {
    const h = highlight;
    if (highlighter && !h) highlighter.reset();
  });
  $effect(() => {
    const hv = hexView;
    if (hexFormatter && !hv) hexFormatter.reset();
  });
  $effect(() => {
    const hv = hexView;
    if (highlighter && hv) highlighter.reset();
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
    // A single character is always a keystroke, including the lone
    // "\r" from the Enter key. Without this guard, every Enter press
    // trips the multi-line paste warning when it's enabled (since
    // "\r" matches the newline regex below), making the terminal
    // feel broken.
    if (data.length < 2) return false;
    if (data.length >= PASTE_THRESHOLD) return true;
    return /[\r\n]/.test(data);
  }

  function sendByte(byte: number) {
    api.sendBytes(new Uint8Array([byte])).catch((e) => onStatus(`send failed: ${e}`));
  }

  async function sendSlow(data: string) {
    const bytes = new TextEncoder().encode(data);
    for (const b of bytes) {
      // Await each send so the next byte doesn't fire until the
      // previous one has been acknowledged by the Go side. Without
      // this, N fire-and-forget calls would queue up concurrently on
      // the Go side and race on the serial port's Write. The per-
      // character delay below isn't enough to serialize them —
      // pasteCharDelayMs is about UART buffer drain pacing, not
      // JS-to-Go call ordering.
      try {
        await api.sendBytes(new Uint8Array([b]));
      } catch (e) {
        onStatus(`send failed: ${e}`);
        return;
      }
      if (localEcho && term) {
        term.write(String.fromCharCode(b));
      }
      if (pasteCharDelayMs > 0) {
        await new Promise((r) => setTimeout(r, pasteCharDelayMs));
      }
    }
  }

  async function handleInput(data: string) {
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
      if (pasteWarnMultiline && multiline && onPasteConfirm) {
        const ok = await onPasteConfirm(data);
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

  // Theme application. lastAppliedThemeId guards against redundant
  // refreshes if the effect re-fires with the same theme reference.
  // options.theme updates cursor + selection chrome immediately;
  // term.refresh repaints already-drawn glyphs in the new palette.
  let lastAppliedThemeId: string | undefined;

  $effect(() => {
    // Read the prop unconditionally so Svelte 5's runtime tracker
    // registers the dependency on the first run (where term may
    // still be null). Without this, the short-circuit in the
    // !term check skips the read and future prop updates don't
    // fire the effect at all.
    const t = theme;
    if (!term || !t || t.id === lastAppliedThemeId) return;
    term.options.theme = themeToXterm(t);
    try {
      term.refresh(0, term.rows - 1);
    } catch {}
    lastAppliedThemeId = t.id;
  });
  // Font size live update.
  //
  // Every "lighter" approach we tried fell short: options.fontSize
  // setter, requestAnimationFrame-deferred fit, window resize
  // dispatch, _charSizeService.measure() via private API — none
  // reliably got xterm to reflow an already-mounted instance. The
  // only path that works across xterm v6 renderers is disposing the
  // instance and creating a fresh one with the new fontSize in its
  // constructor options.
  //
  // Tradeoff: the recreated xterm starts with an empty buffer, so
  // we snapshot plain text from the old buffer and write it back.
  // ANSI color attributes and the current selection are lost in
  // that transition — users changing zoom mid-session pay one
  // flicker and one palette-flatten on the existing scrollback.
  // New output after the resize is fully colored as normal.
  // Seed with the initial prop value so the effect's first run sees
  // fontSize === lastAppliedFontSize and skips the recreate — onMount
  // has already built xterm with this size.
  // Debounce window for rapid zoom presses. Each Cmd+= keypress
  // would otherwise trigger a full xterm teardown/rebuild, which
  // iterates the full scrollback (~10k lines). Collapsing rapid
  // changes into one recreate keeps holding the key or chaining
  // presses from producing a pile of mid-teardown allocations.
  const FONT_RECREATE_DEBOUNCE_MS = 120;
  let lastAppliedFontSize: number = fontSize || 13;
  let pendingFontResize: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    // Read fontSize FIRST so Svelte 5's runtime tracker registers
    // the dependency regardless of whether term is ready yet. A
    // `!term || !fontSize`-style short-circuit would skip the read
    // on the first run (term null) and silently drop tracking.
    const fs = fontSize;
    if (!fs || fs <= 0) return;
    if (!term) return;
    if (fs === lastAppliedFontSize) return;
    lastAppliedFontSize = fs;
    if (pendingFontResize) clearTimeout(pendingFontResize);
    pendingFontResize = setTimeout(() => {
      pendingFontResize = null;
      try {
        recreateWithNewFontSize();
      } catch (e) {
        onStatus(`xterm rebuild failed: ${e}`);
      }
    }, FONT_RECREATE_DEBOUNCE_MS);
  });

  function recreateWithNewFontSize() {
    if (!term) return;

    // Snapshot buffer content as plain text lines.
    const buf = term.buffer.active;
    const lines: string[] = [];
    for (let y = 0; y < buf.length; y++) {
      const line = buf.getLine(y);
      if (line) lines.push(line.translateToString(true));
    }
    while (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();

    const hadFocus = !!term.element && term.element.contains(document.activeElement);

    term.dispose();
    term = null;
    fit = null;

    const fresh = buildTerminal();
    term = fresh.term;
    fit = fresh.fit;

    if (lines.length > 0) {
      term.write(lines.join("\r\n") + "\r\n");
    }
    if (hadFocus) term.focus();
  }

  function buildTerminal(): { term: Terminal; fit: FitAddon } {
    const t = new Terminal({
      fontFamily:
        getComputedStyle(document.documentElement)
          .getPropertyValue("--font-mono")
          .trim() || "SF Mono, Menlo, monospace",
      fontSize: fontSize || 13,
      cursorBlink: true,
      cursorStyle: "block",
      allowProposedApi: true,
      scrollback: 10000,
      convertEol: false,
      theme: theme ? themeToXterm(theme) : fallbackTheme,
      screenReaderMode,
    });
    const f = new FitAddon();
    t.loadAddon(f);
    t.loadAddon(
      new WebLinksAddon((_event, uri) => {
        BrowserOpenURL(uri);
      }),
    );
    t.open(hostEl);
    f.fit();
    t.onData(handleInput);
    t.onSelectionChange(() => {
      if (!copyOnSelect || !t) return;
      const sel = t.getSelection();
      if (sel.length === 0) return;
      navigator.clipboard?.writeText(sel).catch(() => {});
    });
    // After a rebuild the new xterm has the current theme baked in
    // via its constructor options — but the theme-effect guard is
    // keyed to the *last id we applied*, which is still set. Reset
    // it so the next theme change re-runs against the new instance.
    lastAppliedThemeId = t.options.theme && theme ? theme.id : undefined;
    return { term: t, fit: f };
  }
  $effect(() => {
    // Read the prop first so the tracker picks it up regardless of
    // term's state on the first run.
    const sr = screenReaderMode;
    if (term) {
      term.options.screenReaderMode = sr;
    }
  });


  onMount(() => {
    const fresh = buildTerminal();
    term = fresh.term;
    fit = fresh.fit;
    term.focus();

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
    if (pendingFontResize) {
      clearTimeout(pendingFontResize);
      pendingFontResize = null;
    }
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
