import { onMount, onCleanup } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { client } from "@aa/sdk";

function wsUrl(sessionId: string): string {
  const baseUrl = client.getConfig().baseUrl ?? "/api";
  const suffix = `/terminal/${sessionId}`;
  if (baseUrl.startsWith("http://") || baseUrl.startsWith("https://")) {
    const url = new URL(suffix, baseUrl);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    return url.toString();
  }
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}${baseUrl}${suffix}`;
}

export function Xterm(props: { class?: string; sessionId: string }) {
  let containerRef: HTMLDivElement | undefined;
  let term: Terminal | undefined;
  let ws: WebSocket | undefined;

  onMount(() => {
    term = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      theme: {
        background: "#0d1117",
        foreground: "#c9d1d9",
        cursor: "#c9d1d9",
        selectionBackground: "#3b82f6",
        black: "#484f58",
        red: "#ff7b72",
        green: "#3fb950",
        yellow: "#d29922",
        blue: "#58a6ff",
        magenta: "#bc8cff",
        cyan: "#39c5cf",
        white: "#b1bac4",
        brightBlack: "#6e7681",
        brightRed: "#ffa198",
        brightGreen: "#56d364",
        brightYellow: "#e3b341",
        brightBlue: "#79c0ff",
        brightMagenta: "#d2a8ff",
        brightCyan: "#56d4dd",
        brightWhite: "#f0f6fc",
      },
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerRef!);

    ws = new WebSocket(wsUrl(props.sessionId));
    ws.binaryType = "arraybuffer";
    let wsOpened = false;

    ws.onopen = () => {
      wsOpened = true;
      fitAddon.fit();
      sendResize();
    };

    term.onData((data) => {
      if (wsOpened) {
        ws!.send(new TextEncoder().encode(data));
      }
    });

    function sendResize() {
      if (!wsOpened) return;
      const { cols, rows } = term!;
      ws!.send(JSON.stringify({ type: "resize", cols, rows }));
    }

    term.onResize(() => {
      sendResize();
    });

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        term!.write(new Uint8Array(event.data));
      }
    };

    ws.onclose = () => {
      wsOpened = false;
    };

    let resizeTimer: ReturnType<typeof setTimeout>;
    const observer = new ResizeObserver(() => {
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => {
        fitAddon.fit();
        sendResize();
      }, 100);
    });
    observer.observe(containerRef!);

    onCleanup(() => {
      observer.disconnect();
      term!.dispose();
      ws!.close();
    });
  });

  return <div ref={containerRef} class={props.class ?? "h-full w-full"} />;
}
