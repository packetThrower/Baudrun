<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import "@xterm/xterm/css/xterm.css";
  import { themeToXterm, type Theme } from "./api";
  import { highlightLines } from "./highlight";

  type Props = { theme: Theme };
  let { theme }: Props = $props();

  let hostEl: HTMLDivElement;
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let ro: ResizeObserver | null = null;
  // See Terminal.svelte for why this lives at component scope:
  // WebGL atlas cache must be cleared on theme change.
  let webgl: WebglAddon | null = null;

  // Canned output that exercises most highlighter rules: interface names
  // (Cisco full/abbreviated + Junos), up/down/err-disabled/WARNING states,
  // MAC addresses in both formats, IPv4 with CIDR, timestamps, dates.
  const SAMPLE = [
    "RUGGEDCOM RS900G login: admin",
    "Password: ********",
    "",
    "Last login: Fri Apr 17 11:47:05 2026 from 192.168.1.47",
    "RUGGEDCOM RS900G # show interfaces status",
    "",
    "Interface                 Status         VLAN   Duplex   Speed",
    "GigabitEthernet0/1        up             1      full     1000",
    "GigabitEthernet0/2        down           -      -        -",
    "Gi1/0/3                   up             10     full     1000",
    "FastEthernet0/24          err-disabled   -      -        -",
    "ge-0/0/1                  up             -      full     10000",
    "",
    "11:47:12.120  MAC aa:bb:cc:dd:ee:ff learned on Gi1/0/1",
    "11:47:13.440  WARNING: STP learning on Port-channel1",
    "11:47:15.002  Link DOWN on GigabitEthernet0/2",
    "11:47:18.881  Route 192.168.1.47/24 via ge-0/0/1 established",
    "",
    "2026-04-17 11:48:00  Session idle timeout: 00:10:00",
    "RUGGEDCOM RS900G # ",
  ];

  $effect(() => {
    if (term && theme) {
      term.options.theme = themeToXterm(theme);
      // See Terminal.svelte for why we dispose+reattach the WebGL
      // addon on theme change instead of `clearTextureAtlas()`.
      if (webgl && term) {
        webgl.dispose();
        webgl = null;
        try {
          const w = new WebglAddon();
          w.onContextLoss(() => {
            w.dispose();
            webgl = null;
          });
          term.loadAddon(w);
          webgl = w;
        } catch (e) {
          console.warn("WebGL re-init after preview theme change failed, using DOM:", e);
        }
      }
    }
  });

  onMount(() => {
    term = new Terminal({
      fontFamily:
        getComputedStyle(document.documentElement)
          .getPropertyValue("--font-mono")
          .trim() || "SF Mono, Menlo, monospace",
      fontSize: 12,
      cursorBlink: false,
      cursorStyle: "block",
      disableStdin: true,
      scrollback: 0,
      convertEol: false,
      theme: themeToXterm(theme),
    });

    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(hostEl);
    fit.fit();

    // WebGL renderer with DOM fallback — same rationale as
    // Terminal.svelte's buildTerminal. Important here because the
    // theme preview is exactly where renderer-specific color quirks
    // are most visible: if the user is auditioning a theme, we want
    // them to see what the actual session will look like, and the
    // WebGL renderer is what the actual session uses.
    try {
      const w = new WebglAddon();
      w.onContextLoss(() => {
        w.dispose();
        webgl = null;
      });
      term.loadAddon(w);
      webgl = w;
    } catch (e) {
      console.warn("WebGL renderer unavailable in preview, using DOM:", e);
      webgl = null;
    }

    const block = SAMPLE.join("\r\n") + "\r\n";
    term.write(highlightLines(block, false));

    // xterm renders a hollow outlined cursor when the terminal isn't
    // focused and a filled block when focused. Preview should show the
    // filled cursor from the start so users see exactly what the real
    // terminal looks like, without having to click into the modal first.
    term.focus();

    ro = new ResizeObserver(() => {
      try {
        fit?.fit();
      } catch {}
    });
    ro.observe(hostEl);
  });

  onDestroy(() => {
    ro?.disconnect();
    term?.dispose();
    term = null;
  });

  // See Terminal.svelte for the long story. Mirror the same CSS-var
  // backstop here so theme previews look right on Linux / Windows
  // when xterm's runtime stylesheet injection drops.
  const wrapStyle = $derived.by(() => {
    const t = theme;
    const bg = t?.background ?? "#0b0b0d";
    const fg = t?.foreground ?? "#e4e4e7";
    const sel = t?.selection ?? "#1a3a5c";
    const cursor = t?.cursor ?? "#ffffff";
    const cursorAccent = t?.cursorAccent || bg;
    return (
      `background-color:${bg};color:${fg};` +
      `--xterm-sel-bg:${sel};` +
      `--xterm-cursor:${cursor};--xterm-cursor-accent:${cursorAccent};` +
      `--xterm-c0:${t?.black};--xterm-c1:${t?.red};` +
      `--xterm-c2:${t?.green};--xterm-c3:${t?.yellow};` +
      `--xterm-c4:${t?.blue};--xterm-c5:${t?.magenta};` +
      `--xterm-c6:${t?.cyan};--xterm-c7:${t?.white};` +
      `--xterm-c8:${t?.brightBlack};--xterm-c9:${t?.brightRed};` +
      `--xterm-c10:${t?.brightGreen};--xterm-c11:${t?.brightYellow};` +
      `--xterm-c12:${t?.brightBlue};--xterm-c13:${t?.brightMagenta};` +
      `--xterm-c14:${t?.brightCyan};--xterm-c15:${t?.brightWhite};`
    );
  });
</script>

<div class="wrap" style={wrapStyle}>
  <div class="host" bind:this={hostEl}></div>
</div>

<style>
  .wrap {
    width: 100%;
    display: flex;
    padding: 10px 8px 8px 12px;
    overflow: hidden;
    border-radius: var(--radius-md);
  }

  .host {
    flex: 1;
    min-width: 0;
    min-height: 280px;
  }

  :global(.xterm) {
    height: 100%;
  }

  :global(.xterm-viewport) {
    background: transparent !important;
  }

  /* See Terminal.svelte for the long version. Short version: Linux
     WebKit2GTK and Windows WebView2 sometimes drop xterm's runtime
     <style> injection, so we backstop default-foreground color via
     inheritance from .wrap and selection background via a CSS var. */
  :global(.xterm .xterm-selection div) {
    background-color: var(--xterm-sel-bg, transparent) !important;
  }

  :global(.xterm:not(.focus) .xterm-selection div) {
    opacity: 0.6;
  }
</style>
