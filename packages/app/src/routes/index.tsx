import { createFileRoute } from "@tanstack/solid-router";
import { createSignal, createResource, For, Show, onMount } from "solid-js";
import {
  health,
  listSessions,
  listTools,
  deleteSession,
  chat,
  type SseEvent,
  type ChatMsg,
} from "@aa/sdk";

export const Route = createFileRoute("/")({
  component: RouteComponent,
});

// ── Helper: truncate UUID ───────────────────────────────────────

function shortId(id: string) {
  return id.length > 8 ? id.slice(0, 8) : id;
}

// ── Component ──────────────────────────────────────────────────

function RouteComponent() {
  const [h] = createResource(() => health().then((r) => r.data));
  const [tools] = createResource(() => listTools().then((r) => r.data ?? []));
  const [sessions, { refetch }] = createResource(() => listSessions().then((r) => r.data ?? []));

  const [messages, setMessages] = createSignal<ChatMsg[]>([]);
  const [input, setInput] = createSignal("");
  const [streaming, setStreaming] = createSignal(false);
  const [status, setStatus] = createSignal("");
  const [currentId, setCurrentId] = createSignal<string>("");
  const [sidebarOpen, setSidebarOpen] = createSignal(false);

  const threadId = () => currentId() || (crypto.randomUUID() as string);

  async function send() {
    const text = input().trim();
    if (!text || streaming()) return;
    setInput("");
    setStatus("streaming...");

    const userMsg: ChatMsg = { role: "user", content: text };
    setMessages((prev) => [...prev, userMsg]);

    const history = messages()
      .filter((m) => m.role === "user" || m.role === "assistant")
      .concat(userMsg);

    setStreaming(true);
    let replyId = crypto.randomUUID() as string;
    let replyContent = "";
    let toolBuffer: { id: string; name: string; args: string }[] = [];

    setMessages((prev) => [...prev, { role: "assistant", content: "" }]);

    const currentIdValue = threadId() as `${string}-${string}-${string}-${string}-${string}`;
    if (currentIdValue) setCurrentId(currentIdValue);

    try {
      for await (const event of chat(history, tools() || [], currentIdValue)) {
        switch (event.type) {
          case "TEXT_MESSAGE_START":
            replyId = event.messageId as string;
            replyContent = "";
            break;
          case "TEXT_MESSAGE_CONTENT":
            replyContent += event.delta;
            updateLastAssistant(replyContent);
            break;
          case "TOOL_CALL_START":
            setStatus(`🔧 ${event.toolCallName}`);
            toolBuffer.push({ id: event.toolCallId, name: event.toolCallName, args: "" });
            setMessages((prev) => [
              ...prev,
              {
                role: "assistant",
                tool_calls: [
                  {
                    id: event.toolCallId,
                    type: "function",
                    function: { name: event.toolCallName, arguments: "" },
                  },
                ],
              },
            ]);
            break;
          case "TOOL_CALL_ARGS": {
            const tc = toolBuffer[toolBuffer.length - 1];
            if (tc) tc.args += event.delta;
            setMessages((prev) =>
              prev.map((m) => {
                if (m.tool_calls?.[0]?.id === event.toolCallId) {
                  return {
                    ...m,
                    tool_calls: [
                      {
                        ...m.tool_calls[0],
                        function: { ...m.tool_calls[0].function, arguments: tc.args },
                      },
                    ],
                  };
                }
                return m;
              }),
            );
            break;
          }
          case "TOOL_CALL_END":
            setStatus("");
            break;
          case "RUN_ERROR":
            setMessages((prev) => {
              const copy = [...prev];
              if (copy[copy.length - 1]?.role === "assistant") {
                copy[copy.length - 1] = {
                  ...copy[copy.length - 1],
                  content: `Error: ${event.message}`,
                };
              }
              return copy;
            });
            setStatus("error");
            setStreaming(false);
            return;
          case "RUN_FINISHED":
            setStatus("");
            setStreaming(false);
            refetch();
            return;
        }
      }
    } catch (err) {
      setMessages((prev) => {
        const copy = [...prev];
        if (copy[copy.length - 1]?.role === "assistant") {
          copy[copy.length - 1] = { ...copy[copy.length - 1], content: `Error: ${err}` };
        }
        return copy;
      });
      setStatus("error");
    }
    setStreaming(false);
  }

  function updateLastAssistant(content: string) {
    setMessages((prev) => {
      const copy = [...prev];
      for (let i = copy.length - 1; i >= 0; i--) {
        if (copy[i].role === "assistant") {
          copy[i] = { ...copy[i], content };
          return copy;
        }
      }
      return [...prev, { role: "assistant", content }];
    });
  }

  async function deleteSessionById(id: string) {
    const { data } = await deleteSession({ path: { id } });
    if (data) refetch();
  }

  async function newSession() {
    setCurrentId("");
    setMessages([]);
    setStatus("");
    setStreaming(false);
  }

  async function resumeSession(id: string) {
    setCurrentId(id);
    setSidebarOpen(false);
    setMessages([]);
  }

  return (
    <div class="h-screen flex flex-col">
      {/* ── Header ────────────────────────────────────────────── */}
      <header class="flex items-center justify-between px-4 py-2 border-b border-[#30363d] bg-[#161b22] shrink-0">
        <div class="flex items-center gap-3">
          <button
            class="text-[#3b82f6] hover:text-[#60a5fa] text-sm"
            onClick={() => setSidebarOpen((o) => !o)}
          >
            ☰
          </button>
          <span class="font-semibold text-[#c9d1d9]">AAgent</span>
          {h() && (
            <span class="text-xs text-[#555]">
              {h()!.provider}/{h()!.model}
            </span>
          )}
        </div>
        <div class="flex items-center gap-3 text-xs">
          <Show when={h()}>
            <span class="text-green">● {h()!.tool_count} tools</span>
          </Show>
          <button class="text-[#3b82f6] hover:text-[#60a5fa]" onClick={newSession}>
            + New
          </button>
          <a href="/terminal" class="text-[#3b82f6] hover:text-[#60a5fa] text-xs">
            Terminal
          </a>
        </div>
      </header>

      {/* ── Body ──────────────────────────────────────────────── */}
      <div class="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <Show when={sidebarOpen()}>
          <aside class="w-64 shrink-0 border-r border-[#30363d] bg-[#0d1117] flex flex-col overflow-hidden">
            <div class="p-3 border-b border-[#30363d] text-xs text-[#555] font-semibold">
              Sessions
            </div>
            <div class="flex-1 overflow-y-auto p-2">
              <For each={sessions()}>
                {(s) => (
                  <div
                    class="flex items-center gap-1 px-2 py-1 hover:bg-[#161b22] rounded cursor-pointer group"
                    onClick={() => resumeSession(s.session_id)}
                  >
                    <span class="text-xs text-[#3b82f6] font-mono">{shortId(s.session_id)}</span>
                    <span class="text-xs text-[#555] flex-1 truncate">{s.model}</span>
                    <span class="text-xs text-[#555]">{s.message_count}</span>
                    <button
                      class="text-[#f87171] opacity-0 group-hover:opacity-100 text-xs"
                      onClick={(e) => {
                        e.stopPropagation();
                        deleteSessionById(s.session_id);
                      }}
                    >
                      ×
                    </button>
                  </div>
                )}
              </For>
            </div>
          </aside>
        </Show>

        {/* ── Chat area ──────────────────────────────────────── */}
        <div class="flex-1 flex flex-col overflow-hidden">
          {/* Messages */}
          <div class="flex-1 overflow-y-auto p-4 space-y-4" id="chat-messages">
            <Show
              when={messages().length === 0}
              fallback={
                <For each={messages()}>
                  {(msg) => (
                    <div class={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}>
                      <div
                        classList={{
                          "max-w-[80%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap break-words": true,
                          "bg-[#1f6feb] text-white": msg.role === "user",
                          "bg-[#161b22] border border-[#30363d] text-[#c9d1d9]":
                            msg.role === "assistant" && !msg.tool_calls,
                        }}
                      >
                        <Show when={msg.tool_calls}>
                          <div class="flex items-center gap-2 text-xs text-yellow">
                            <span>🔧 {msg.tool_calls![0].function.name}</span>
                            <span class="text-[#555]">
                              {msg.tool_calls![0].function.arguments.length > 60
                                ? msg.tool_calls![0].function.arguments.slice(0, 60) + "..."
                                : msg.tool_calls![0].function.arguments}
                            </span>
                          </div>
                        </Show>
                        <Show when={msg.content}>{msg.content}</Show>
                      </div>
                    </div>
                  )}
                </For>
              }
            >
              <div class="flex items-center justify-center h-full">
                <div class="text-center">
                  <p class="text-[#555] text-sm">Connected. Type a message to start.</p>
                  <Show when={h()}>
                    <p class="text-[#444] text-xs mt-1">{h()!.tool_count} tools available</p>
                  </Show>
                </div>
              </div>
            </Show>
          </div>

          {/* Status bar */}
          <Show when={status()}>
            <div class="px-4 py-1 text-xs text-[#666] border-t border-[#30363d]">{status()}</div>
          </Show>

          {/* Input */}
          <div class="border-t border-[#30363d] p-4 bg-[#0d1117]">
            <div class="flex gap-2 max-w-4xl mx-auto">
              <input
                class="flex-1 bg-[#161b22] border border-[#30363d] rounded-lg px-3 py-2 text-sm text-[#c9d1d9] placeholder-[#555] outline-none focus:border-[#3b82f6]"
                placeholder="Type a message..."
                value={input()}
                onInput={(e) => setInput(e.currentTarget.value)}
                onKeyDown={(e) => e.key === "Enter" && send()}
                disabled={streaming()}
              />
              <button
                class="bg-[#1f6feb] text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-[#388bfd] disabled:opacity-50"
                onClick={send}
                disabled={streaming() || !input().trim()}
              >
                Send
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
