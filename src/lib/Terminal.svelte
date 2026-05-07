<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { SerializeAddon } from "@xterm/addon-serialize";
  import "@xterm/xterm/css/xterm.css";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, themeToXterm, type Theme } from "./api";
  import { TerminalHighlighter } from "./highlight";
  import { HexFormatter } from "./hexdump";
  import { rulesVersion } from "../stores/highlight";

  type Props = {
    lineEnding?: "cr" | "lf" | "crlf";
    localEcho?: boolean;
    theme?: Theme | undefined;
    fontSize?: number;
    scrollback?: number;
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
    scrollback = 10000,
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
  // SerializeAddon is what lets recreateTerminal round-trip the
  // existing buffer through an ANSI-encoded snapshot on font-size
  // / scrollback changes, preserving syntax-highlighting SGR
  // attributes across the disposal + rebuild. Without it we'd have
  // to fall back to translateToString(true) which strips colors.
  let serializer: SerializeAddon | null = null;
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

  // Live re-render of the existing scrollback when the user changes
  // anything that affects how text gets transformed: the highlight
  // toggle, the timestamps toggle, or the active rule set (Settings →
  // Highlighting checkboxes — recompiling the rules bumps
  // $rulesVersion). Without this, only newly-arrived text reflects
  // the new settings; existing scrollback stays stale.
  //
  // The first run is skipped via `replayPrimed` because we don't want
  // to replay during initial mount (the rawHistory is empty anyway,
  // and a snapshot may have been written into xterm by the multi-
  // window migration path). Replay is also a no-op when rawHistory is
  // empty — see `TerminalHighlighter.replay()` for why migrated
  // sessions must not be wiped on toggle.
  let replayPrimed = false;
  $effect(() => {
    // Read the reactive deps unconditionally so Svelte 5's tracker
    // picks them up on the first run regardless of which branch we
    // hit. Same hazard pattern as the theme effect above.
    const h = highlight;
    const ts = timestamps;
    const _v = $rulesVersion;
    void _v;
    if (!replayPrimed) {
      replayPrimed = true;
      return;
    }
    // hexView is its own pipeline — replaying through the highlight
    // path while hex is active would scramble the rendered output.
    // The hexView toggle above already resets the hex formatter; when
    // the user flips back out of hex, new data picks up the current
    // highlight settings naturally so retroactive replay isn't needed.
    if (!term || !highlighter || hexView) return;
    if (!highlighter.hasHistory()) return;
    const replay = highlighter.replay(h, ts);
    // SerializeAddon's snapshot can leave the cursor wherever the
    // last frame ended; reset() puts us back at row 0 col 0 and
    // clears scrollback. Then write the replayed history. Focus is
    // preserved by xterm across reset.
    term.reset();
    if (replay.length > 0) term.write(replay);
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

  // Slow-paste abort: Esc during an in-flight sendSlow() sets this
  // flag; the loop checks it each iteration. Also backs the "Esc to
  // cancel" progress pill below.
  let pasteSending = $state(false);
  let pasteSent = $state(0);
  let pasteTotal = $state(0);
  let pasteAbortRequested = false;

  async function sendSlow(data: string) {
    const bytes = new TextEncoder().encode(data);
    pasteSending = true;
    pasteAbortRequested = false;
    pasteSent = 0;
    pasteTotal = bytes.length;
    try {
      for (const b of bytes) {
        if (pasteAbortRequested) {
          onStatus(`Paste aborted after ${pasteSent}/${bytes.length} bytes`);
          return;
        }
        // Await each send so the next byte doesn't fire until the
        // previous one has been acknowledged by the backend. Without
        // this, N fire-and-forget calls would queue up concurrently
        // and race on the serial port's Write. The per-character
        // delay below isn't enough to serialize them —
        // pasteCharDelayMs is about UART buffer drain pacing, not
        // JS-to-backend call ordering.
        try {
          await api.sendBytes(new Uint8Array([b]));
        } catch (e) {
          onStatus(`send failed: ${e}`);
          return;
        }
        pasteSent += 1;
        if (localEcho && term) {
          term.write(String.fromCharCode(b));
        }
        if (pasteCharDelayMs > 0) {
          await new Promise((r) => setTimeout(r, pasteCharDelayMs));
        }
      }
    } finally {
      pasteSending = false;
    }
  }

  async function handleInput(data: string) {
    // Esc during an in-flight slow-paste aborts it. Swallow the Esc
    // so the device doesn't see a stray escape mid-stream.
    if (pasteSending && data === "\x1b") {
      pasteAbortRequested = true;
      return;
    }

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
        // The confirm modal pulled focus away from xterm; always
        // return it here regardless of accept/cancel so the cursor
        // stays live and the user doesn't have to click back in.
        term?.focus();
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
  // The scrollback is snapshotted through @xterm/addon-serialize
  // before disposal (see recreateTerminal), which re-emits SGR
  // sequences for each styled region — so syntax-highlight colors
  // survive the rebuild. The current selection is not serialised
  // and is lost across zoom; that's the only remaining visual
  // glitch users pay for a mid-session resize.
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
        recreateTerminal();
      } catch (e) {
        onStatus(`xterm rebuild failed: ${e}`);
      }
    }, FONT_RECREATE_DEBOUNCE_MS);
  });

  // Scrollback live update. Same recreate path as font-size — xterm
  // doesn't resize the ring buffer in place, so changing this tears
  // down and rebuilds the instance. Since scrollback changes come
  // from a dropdown selection (not rapid keyboard presses), no
  // debounce is needed here.
  let lastAppliedScrollback: number = scrollback || 10000;

  $effect(() => {
    const sb = scrollback;
    if (!sb || sb <= 0) return;
    if (!term) return;
    if (sb === lastAppliedScrollback) return;
    lastAppliedScrollback = sb;
    try {
      recreateTerminal();
    } catch (e) {
      onStatus(`xterm rebuild failed: ${e}`);
    }
  });

  function recreateTerminal() {
    if (!term) return;

    // Snapshot buffer content as an ANSI-encoded string. SerializeAddon
    // walks the cell grid and emits SGR escape sequences for every
    // styled region it encounters, so colors from the syntax-highlight
    // pipeline survive the xterm dispose/rebuild. Falls back to a
    // plain-text translateToString sweep if serialization throws (the
    // addon occasionally trips on unusual buffer states) so we don't
    // blank the scrollback either way.
    let snapshot = "";
    try {
      if (serializer) {
        snapshot = serializer.serialize({ scrollback: term.buffer.active.length });
      }
    } catch (e) {
      onStatus(`serialize fallback: ${e}`);
    }
    if (!snapshot) {
      const buf = term.buffer.active;
      const lines: string[] = [];
      for (let y = 0; y < buf.length; y++) {
        const line = buf.getLine(y);
        if (line) lines.push(line.translateToString(true));
      }
      while (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();
      snapshot = lines.join("\r\n");
    }

    const hadFocus = !!term.element && term.element.contains(document.activeElement);

    term.dispose();
    term = null;
    fit = null;
    serializer = null;

    const fresh = buildTerminal();
    term = fresh.term;
    fit = fresh.fit;
    serializer = fresh.serializer;

    if (snapshot.length > 0) {
      // Write the snapshot verbatim — SerializeAddon already leaves
      // the cursor where the prompt naturally ends, so appending an
      // extra \r\n here would bump the prompt a row down every time
      // the user hits Cmd +/-.
      term.write(snapshot);
    }
    if (hadFocus) term.focus();
  }

  function buildTerminal(): { term: Terminal; fit: FitAddon; serializer: SerializeAddon } {
    const t = new Terminal({
      fontFamily:
        getComputedStyle(document.documentElement)
          .getPropertyValue("--font-mono")
          .trim() || "SF Mono, Menlo, monospace",
      fontSize: fontSize || 13,
      cursorBlink: true,
      cursorStyle: "block",
      allowProposedApi: true,
      scrollback: scrollback || 10000,
      convertEol: false,
      theme: theme ? themeToXterm(theme) : fallbackTheme,
      screenReaderMode,
    });
    const f = new FitAddon();
    const s = new SerializeAddon();
    t.loadAddon(f);
    t.loadAddon(s);
    t.loadAddon(
      new WebLinksAddon((_event, uri) => {
        void openUrl(uri);
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
    return { term: t, fit: f, serializer: s };
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
    serializer = fresh.serializer;
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

  // Build the inline-style string fed to .wrap. The CSS variables it
  // sets back the :global rules in the style block below — they're
  // the Linux / Windows backstop for xterm's runtime-injected per-color
  // stylesheet, which sometimes silently drops on those renderers.
  // See the long comment near :global(.xterm .xterm-fg-0).
  // background-color and color are folded into the same string so the
  // wrap has a single source of truth — mixing `style:` directives with
  // a `style=` attribute can produce surprising precedence depending on
  // Svelte's update path.
  const wrapStyle = $derived.by(() => {
    const t = theme;
    const bg = t?.background ?? "#0b0b0d";
    const fg = t?.foreground ?? "#e4e4e7";
    const sel = t?.selection ?? "#1a3a5c";
    const cursor = t?.cursor ?? "#ffffff";
    const cursorAccent = t?.cursorAccent || bg;
    const c0 = t?.black ?? "#1e1e22";
    const c1 = t?.red ?? "#ff6961";
    const c2 = t?.green ?? "#7cd992";
    const c3 = t?.yellow ?? "#f5d76e";
    const c4 = t?.blue ?? "#6cb6ff";
    const c5 = t?.magenta ?? "#d794ff";
    const c6 = t?.cyan ?? "#7ce0e0";
    const c7 = t?.white ?? "#d4d4d8";
    const c8 = t?.brightBlack ?? "#4a4a52";
    const c9 = t?.brightRed ?? "#ff8a80";
    const c10 = t?.brightGreen ?? "#a2e5b3";
    const c11 = t?.brightYellow ?? "#fce488";
    const c12 = t?.brightBlue ?? "#94ccff";
    const c13 = t?.brightMagenta ?? "#e5b6ff";
    const c14 = t?.brightCyan ?? "#a6ecec";
    const c15 = t?.brightWhite ?? "#ffffff";
    return (
      `background-color:${bg};color:${fg};` +
      `--xterm-sel-bg:${sel};` +
      `--xterm-cursor:${cursor};--xterm-cursor-accent:${cursorAccent};` +
      `--xterm-c0:${c0};--xterm-c1:${c1};--xterm-c2:${c2};--xterm-c3:${c3};` +
      `--xterm-c4:${c4};--xterm-c5:${c5};--xterm-c6:${c6};--xterm-c7:${c7};` +
      `--xterm-c8:${c8};--xterm-c9:${c9};--xterm-c10:${c10};--xterm-c11:${c11};` +
      `--xterm-c12:${c12};--xterm-c13:${c13};--xterm-c14:${c14};--xterm-c15:${c15};`
    );
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
    // Drop the raw history too — if we kept it, toggling highlight
    // after a clear would resurrect the cleared scrollback via the
    // replay path. The user's mental model of "Clear" is "wipe
    // everything in this terminal", not "wipe the visible bit".
    highlighter?.clearHistory();
    hexFormatter?.reset();
  }

  export function refit() {
    try {
      fit?.fit();
    } catch {}
  }

  /** Serialize the current xterm buffer to an ANSI-escape-encoded
   *  string — same machinery recreateTerminal uses for zoom-in-place.
   *  Used by multi-window session migration to carry visible
   *  scrollback over to the new window. Returns "" if the terminal
   *  isn't mounted yet. */
  export function snapshot(): string {
    if (!serializer || !term) return "";
    try {
      return serializer.serialize({ scrollback: term.buffer.active.length });
    } catch (e) {
      console.warn("terminal snapshot failed:", e);
      return "";
    }
  }

  /** Write a previously-serialized buffer into a freshly mounted
   *  terminal. Called on a new window's mount when the backend has
   *  a pending snapshot from a session-migration source. The string
   *  comes from snapshot() / SerializeAddon, which leaves the cursor
   *  parked at the active prompt position; no extra newline needed. */
  export function restoreSnapshot(data: string) {
    if (!data || !term) return;
    term.write(data);
  }
</script>

<div class="wrap" style={wrapStyle}>
  <div class="host" bind:this={hostEl}></div>
  {#if pasteSending}
    <div
      class="paste-progress"
      role="status"
      aria-live="polite"
      aria-label="Pasting {pasteSent} of {pasteTotal} bytes. Press Escape to cancel."
      style:color={theme?.foreground ?? "rgba(255,255,255,0.95)"}
      style:background={theme?.selection ?? "rgba(0,0,0,0.55)"}
    >
      <span class="paste-progress-label">Paste</span>
      <span class="paste-progress-count">{pasteSent}/{pasteTotal} bytes</span>
      <span class="paste-progress-hint">Esc to cancel</span>
    </div>
  {/if}
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    padding: 10px 8px 8px 12px;
    overflow: hidden;
    position: relative;
  }

  /* Floating paste-progress pill. Top-right of the terminal,
     positioned over the xterm but with pointer-events:none so it
     never swallows clicks meant for the terminal surface. Color
     and background come from the active theme (foreground + selection)
     so it reads correctly on every palette. */
  .paste-progress {
    position: absolute;
    top: 12px;
    right: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-radius: 999px;
    font: 500 11px/1.2 var(--font-ui, system-ui, sans-serif);
    letter-spacing: 0.01em;
    pointer-events: none;
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
    user-select: none;
    z-index: 2;
  }

  .paste-progress-label {
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.08em;
    opacity: 0.7;
  }

  .paste-progress-count {
    font-variant-numeric: tabular-nums;
    font-family: var(--font-mono, Menlo, monospace);
    font-size: 12px;
  }

  .paste-progress-hint {
    opacity: 0.6;
    font-size: 10px;
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

  /* Linux (WebKit2GTK) and Windows (WebView2) sometimes drop xterm's
     runtime-injected stylesheet — the one that carries every theme-
     derived rule: default foreground on .xterm-rows, selection
     background on .xterm-selection div, the 16 ANSI color classes
     (.xterm-fg-0..15 / .xterm-bg-0..15), and the cursor's block fill.
     macOS WebKit doesn't have this quirk; rendering works without any
     of these fallbacks.

     When the injection drops:
       * default-fg spans inherit color from body (var(--fg-primary)),
         making text invisible on dark themes;
       * .xterm-fg-N classes lose their per-color rule, so syntax-
         highlighted output (every CSI 3Nm escape from the highlight
         packs) collapses to default foreground — Cisco / Junos packs
         look unhighlighted;
       * the selection highlight has no fill;
       * the cursor block has no fill.

     wrapStyle in the script block sets the full theme palette as CSS
     variables on .wrap (--xterm-c0..15, --xterm-cursor, --xterm-sel-bg,
     etc.). The rules below route those variables into the right
     internal xterm nodes. !important is load-bearing: xterm's
     injected rule, when it does run, targets the same selectors at
     equal specificity. Color values match xterm's output exactly so
     macOS is visually unchanged. */
  :global(.xterm .xterm-selection div) {
    background-color: var(--xterm-sel-bg, transparent) !important;
  }

  /* Inactive selection (terminal not focused) — slightly muted.
     xterm normally uses a separate `selectionInactiveBackground` color;
     we don't track that distinction in our theme schema, so ramp the
     same color down via opacity. The :not(.focus) selector mirrors
     xterm's own focus class. */
  :global(.xterm:not(.focus) .xterm-selection div) {
    opacity: 0.6;
  }

  /* ANSI palette — every CSI 3Nm and CSI 4Nm SGR escape lands in
     one of these classes. xterm v6 uses the same numbering for both
     the standard 8 (0-7) and bright 8 (8-15). 16 fg + 16 bg rules. */
  :global(.xterm .xterm-fg-0)  { color: var(--xterm-c0)  !important; }
  :global(.xterm .xterm-fg-1)  { color: var(--xterm-c1)  !important; }
  :global(.xterm .xterm-fg-2)  { color: var(--xterm-c2)  !important; }
  :global(.xterm .xterm-fg-3)  { color: var(--xterm-c3)  !important; }
  :global(.xterm .xterm-fg-4)  { color: var(--xterm-c4)  !important; }
  :global(.xterm .xterm-fg-5)  { color: var(--xterm-c5)  !important; }
  :global(.xterm .xterm-fg-6)  { color: var(--xterm-c6)  !important; }
  :global(.xterm .xterm-fg-7)  { color: var(--xterm-c7)  !important; }
  :global(.xterm .xterm-fg-8)  { color: var(--xterm-c8)  !important; }
  :global(.xterm .xterm-fg-9)  { color: var(--xterm-c9)  !important; }
  :global(.xterm .xterm-fg-10) { color: var(--xterm-c10) !important; }
  :global(.xterm .xterm-fg-11) { color: var(--xterm-c11) !important; }
  :global(.xterm .xterm-fg-12) { color: var(--xterm-c12) !important; }
  :global(.xterm .xterm-fg-13) { color: var(--xterm-c13) !important; }
  :global(.xterm .xterm-fg-14) { color: var(--xterm-c14) !important; }
  :global(.xterm .xterm-fg-15) { color: var(--xterm-c15) !important; }
  :global(.xterm .xterm-bg-0)  { background-color: var(--xterm-c0)  !important; }
  :global(.xterm .xterm-bg-1)  { background-color: var(--xterm-c1)  !important; }
  :global(.xterm .xterm-bg-2)  { background-color: var(--xterm-c2)  !important; }
  :global(.xterm .xterm-bg-3)  { background-color: var(--xterm-c3)  !important; }
  :global(.xterm .xterm-bg-4)  { background-color: var(--xterm-c4)  !important; }
  :global(.xterm .xterm-bg-5)  { background-color: var(--xterm-c5)  !important; }
  :global(.xterm .xterm-bg-6)  { background-color: var(--xterm-c6)  !important; }
  :global(.xterm .xterm-bg-7)  { background-color: var(--xterm-c7)  !important; }
  :global(.xterm .xterm-bg-8)  { background-color: var(--xterm-c8)  !important; }
  :global(.xterm .xterm-bg-9)  { background-color: var(--xterm-c9)  !important; }
  :global(.xterm .xterm-bg-10) { background-color: var(--xterm-c10) !important; }
  :global(.xterm .xterm-bg-11) { background-color: var(--xterm-c11) !important; }
  :global(.xterm .xterm-bg-12) { background-color: var(--xterm-c12) !important; }
  :global(.xterm .xterm-bg-13) { background-color: var(--xterm-c13) !important; }
  :global(.xterm .xterm-bg-14) { background-color: var(--xterm-c14) !important; }
  :global(.xterm .xterm-bg-15) { background-color: var(--xterm-c15) !important; }

  /* Block cursor backstop. We set cursorStyle: "block" in
     buildTerminal, so we only need to handle the block variant. */
  :global(.xterm .xterm-cursor.xterm-cursor-block) {
    background-color: var(--xterm-cursor) !important;
    color: var(--xterm-cursor-accent) !important;
  }
</style>
