<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";
  import { themeToXterm, type Theme } from "./api";
  import { highlightLines } from "./highlight";

  type Props = { theme: Theme };
  let { theme }: Props = $props();

  let hostEl: HTMLDivElement;
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let ro: ResizeObserver | null = null;

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
</script>

<div
  class="wrap"
  style:background-color={theme?.background ?? "#0b0b0d"}
  style:color={theme?.foreground ?? "#e4e4e7"}
  style:--xterm-sel-bg={theme?.selection ?? "#1a3a5c"}
>
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
