import {
  type Component,
  createSignal,
  createResource,
  For,
  Show,
  onMount,
  createMemo,
} from "solid-js";
import { useTerminalDimensions } from "@opentui/solid";
import type { SessionSummary } from "@aa/sdk";
import { health, listSessions, deleteSession } from "@aa/sdk";
import { useDialog } from "../ui/dialog";
import { ConfigDialog } from "../component/dialog/config";
import { SessionListDialog } from "../component/dialog/session-list";
import { WorkspaceListDialog } from "../component/dialog/workspace-list";
import { CommandPalette, type PaletteCommand } from "../component/command-palette";
import { useToast } from "../ui/toast";
import { useWorkspace } from "../context/workspace";
import { Prompt, type PromptRef } from "../component/prompt";

interface HomeProps {
  onStart: () => void;
  onContinue: (sessionId: string) => void;
}

function DeleteConfirm(props: { sessionId: string; onConfirm: () => void; onCancel: () => void }) {
  return (
    <box flexDirection="column" padding={1}>
      <text fg="#f87171">Delete session?</text>
      <box height={1} />
      <text fg="#888">Session: {props.sessionId.slice(0, 8)}</text>
      <text fg="#888">This cannot be undone.</text>
      <box height={1} />
      <box flexDirection="row" justifyContent="flex-end">
        <box backgroundColor="#f87171" paddingLeft={1} paddingRight={1} on:press={props.onConfirm}>
          <text fg="#000"> Delete </text>
        </box>
        <box width={2} />
        <box paddingLeft={1} paddingRight={1} on:press={props.onCancel}>
          <text fg="#888">Cancel</text>
        </box>
      </box>
    </box>
  );
}

export const Home: Component<HomeProps> = (props) => {
  const [connected, setConnected] = createSignal(false);
  const [toolCount, setToolCount] = createSignal(0);
  const [provider, setProvider] = createSignal("");
  const [model, setModel] = createSignal("");
  const [sessions, { refetch }] = createResource(() => listSessions().then((r) => r.data ?? []));
  const dialog = useDialog();
  const toast = useToast();

  const [mcpCount, setMcpCount] = createSignal(0);
  const ws = useWorkspace();

  onMount(async () => {
    try {
      const { data: h } = await health();
      if (h) {
        setConnected(true);
        setToolCount(h.tool_count);
        setProvider(h.provider);
        setModel(h.model);
        setMcpCount(h.mcp);
      }
    } catch {
      setConnected(false);
    }
  });

  const recentSessions = () => {
    const s = sessions();
    return s ? s.slice(0, 5) : [];
  };

  const openConfig = () => {
    dialog.push(() => (
      <ConfigDialog
        current={{ provider: provider(), model: model(), baseUrl: "", hasApiKey: false }}
        onSave={(c) => {
          setProvider(c.provider);
          setModel(c.model);
          toast.show({ message: `Config: ${c.provider}/${c.model}`, variant: "info" });
        }}
      />
    ));
  };

  const deleteSessionFn = async (id: string) => {
    const { data } = await deleteSession({ path: { id } });
    if (data) {
      toast.show({ message: "Session deleted", variant: "info" });
      refetch();
    } else {
      toast.show({ message: "Failed to delete session", variant: "error" });
    }
  };

  const openPalette = () => {
    dialog.push(() => (
      <CommandPalette
        onCommand={(cmd: PaletteCommand) => {
          switch (cmd.action) {
            case "config":
              openConfig();
              break;
            case "session-list":
              dialog.push(() => <SessionListDialog onSelect={(id) => props.onContinue(id)} />);
              break;
            case "delete-session":
              dialog.push(() => (
                <SessionListDialog
                  onSelect={async (id) => {
                    const { data } = await deleteSession({ path: { id } });
                    toast.show({
                      message: data ? "Session deleted" : "Delete failed",
                      variant: data ? "info" : "error",
                    });
                    refetch();
                  }}
                />
              ));
              break;
            case "workspace":
              dialog.push(() => <WorkspaceListDialog />);
              break;
            case "back-home":
              /* already home */ break;
            case "new-session":
              props.onStart();
              break;
            case "help":
              toast.show({ message: "Ctrl+K: Palette · Esc: Close · ↑↓: Select", variant: "info" });
              break;
          }
        }}
      />
    ));
  };

  const dimensions = useTerminalDimensions();
  const promptMaxWidth = createMemo(() => Math.max(75, Math.floor(dimensions().width * 0.7)));
  const version = "v0.1.0";

  const placeholder = {
    normal: [
      "Fix a TODO in the codebase",
      "What is the tech stack?",
      "Explain this code",
      "Write a test",
      "Bump the dependency",
    ],
  };

  return (
    <box width="100%" height="100%" flexDirection="column">
      {/* Header */}
      <box height={1} flexDirection="row" paddingLeft={2} paddingRight={2}>
        <text fg="cyan">AAgent</text>
        <box width={1} />
        <Show when={connected()}>
          <text fg="#555">
            {provider()}/{model()}
          </text>
        </Show>
        <box flexGrow={1} />
        {connected() ? (
          <text fg="green">● {toolCount()} tools</text>
        ) : (
          <text fg="red">● Disconnected</text>
        )}
        <box width={1} />
        <box on:press={openPalette}>
          <text fg="#3b82f6">[Ctrl+K]</text>
        </box>
        <box width={1} />
        <box on:press={openConfig}>
          <text fg="#3b82f6">[Config]</text>
        </box>
      </box>

      {/* Main content */}
      <box flexGrow={1} alignItems="center" paddingLeft={2} paddingRight={2}>
        <box flexGrow={1} minHeight={0} />
        <box flexShrink={0}>
          <text fg="cyan">AAgent – AI Assistant</text>
        </box>
        <box height={1} minHeight={0} flexShrink={1} />

        <Show when={recentSessions().length > 0}>
          <text fg="#888">Recent sessions:</text>
          <For each={recentSessions()}>
            {(s) => (
              <box flexDirection="row">
                <box
                  flexGrow={1}
                  flexDirection="row"
                  on:press={() => props.onContinue(s.session_id)}
                >
                  <text fg="#aaa">{s.session_id.slice(0, 8)}</text>
                  <text fg="#666">
                    {" "}
                    {s.model} · {s.message_count} msgs
                  </text>
                </box>
                <box
                  on:press={() => {
                    dialog.push(() => (
                      <DeleteConfirm
                        sessionId={s.session_id}
                        onConfirm={() => {
                          dialog.pop();
                          deleteSessionFn(id);
                        }}
                        onCancel={() => dialog.pop()}
                      />
                    ));
                  }}
                >
                  <text fg="#f87171">[×]</text>
                </box>
              </box>
            )}
          </For>
          <box height={2} />
        </Show>

        <box width="100%" maxWidth={promptMaxWidth()} zIndex={1000} paddingTop={1} flexShrink={0}>
          <Prompt
            placeholders={placeholder}
            model={model()}
            provider={provider()}
            onSubmit={() => props.onStart()}
          />
        </box>

        <box flexGrow={1} minHeight={0} />
      </box>

      {/* Footer — matches opencode home_footer: [Directory] [MCP] spacer [Version] */}
      <box
        width="100%"
        paddingTop={1}
        paddingBottom={1}
        paddingLeft={2}
        paddingRight={2}
        flexDirection="row"
        flexShrink={0}
        gap={2}
      >
        <text fg="#555">
          {ws.directory().replace(process.env.HOME || "", "~")}
          {ws.branch() ? `:${ws.branch()}` : ""}
        </text>
        <Show when={mcpCount() > 0}>
          <text fg="#555">⊙ {mcpCount()} MCP</text>
        </Show>
        <box flexGrow={1} />
        <text fg="#555">{version}</text>
      </box>
    </box>
  );
};
