import { createSignal, For, Show } from "solid-js";
import { Xterm } from "./terminal";
import { client } from "@aa/sdk";

interface Tab {
  id: string;
  label: string;
}

function generateLabel(existing: Tab[]): string {
  let n = 1;
  while (existing.some((t) => t.label === `Terminal ${n}`)) n++;
  return `Terminal ${n}`;
}

function api(path: string): string {
  const base = client.getConfig().baseUrl ?? "/api";
  return `${base}${path}`;
}

export function TerminalTabs() {
  const [tabs, setTabs] = createSignal<Tab[]>([]);
  const [activeId, setActiveId] = createSignal<string | null>(null);

  async function addTab() {
    const res = await fetch(api("/terminals"), { method: "POST" });
    if (!res.ok) return;
    const { id } = await res.json();
    const tab: Tab = { id, label: generateLabel(tabs()) };
    setTabs((prev) => [...prev, tab]);
    setActiveId(id);
  }

  async function closeTab(e: MouseEvent, id: string) {
    e.stopPropagation();
    await fetch(api(`/terminals/${id}`), { method: "DELETE" });
    const remaining = tabs().filter((t) => t.id !== id);
    setTabs(remaining);
    if (activeId() === id) {
      setActiveId(remaining.length > 0 ? remaining[remaining.length - 1].id : null);
    }
  }

  return (
    <div class="h-full flex flex-col">
      <Show
        when={tabs().length > 0}
        fallback={
          <div class="flex-1 flex items-center justify-center bg-[#0d1117] text-[#8b949e]">
            <button
              onClick={addTab}
              class="px-4 py-2 rounded border border-[#30363d] hover:bg-[#21262d] text-sm cursor-pointer"
            >
              + New Terminal
            </button>
          </div>
        }
      >
        <div class="flex items-center bg-[#1c2128] border-b border-[#30363d] shrink-0 overflow-x-auto text-xs">
          <For each={tabs()}>
            {(tab) => (
              <div
                onClick={() => setActiveId(tab.id)}
                class={`
                  flex items-center gap-2 px-3 py-1.5 border-r border-[#30363d] cursor-pointer select-none shrink-0
                  ${activeId() === tab.id ? "bg-[#0d1117] text-[#c9d1d9]" : "bg-[#1c2128] text-[#8b949e] hover:text-[#c9d1d9]"}
                `}
              >
                <span>{tab.label}</span>
                <button
                  onClick={(e) => closeTab(e, tab.id)}
                  class="text-[#8b949e] hover:text-[#f85149] text-xs leading-none cursor-pointer"
                >
                  x
                </button>
              </div>
            )}
          </For>
          <button
            onClick={addTab}
            class="px-2 py-1.5 text-[#8b949e] hover:text-[#c9d1d9] cursor-pointer shrink-0"
          >
            +
          </button>
        </div>

        <div class="flex-1 relative">
          <For each={tabs()}>
            {(tab) => (
              <div
                class="absolute inset-0"
                style={{ display: activeId() === tab.id ? "block" : "none" }}
              >
                <Xterm sessionId={tab.id} />
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
