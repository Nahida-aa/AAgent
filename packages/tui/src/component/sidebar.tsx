import { type Component, createResource, For, Show } from "solid-js";
import { health, listSessions } from "@aa/sdk";

interface SidebarProps {
  currentSessionId?: string;
  onNewSession: () => void;
  onSelectSession: (id: string) => void;
  onBack: () => void;
}

const SIDEBAR_WIDTH = 28;

export const Sidebar: Component<SidebarProps> = (props) => {
  const [healthData] = createResource(() => health().then((r) => r.data));
  const [sessions] = createResource(() => listSessions().then((r) => r.data ?? []));

  const h = () => healthData();
  const sessionList = () => sessions() ?? [];

  return (
    <box width={SIDEBAR_WIDTH} height="100%" flexDirection="column" backgroundColor="#0d1117">
      {/* Brand */}
      <box height={1} flexDirection="row" paddingLeft={1}>
        <text fg="cyan">AAgent</text>
        <box flexGrow={1} />
        <box on:press={props.onBack}>
          <text fg="#555">[←]</text>
        </box>
      </box>

      <box height={1} />

      {/* Config section */}
      <box flexDirection="column" paddingLeft={1} paddingRight={1}>
        <text fg="#555">Config</text>
        <Show when={h()}>
          <text fg="#888">
            {h()!.provider}/{h()!.model}
          </text>
        </Show>
        <Show when={h()}>
          <text fg="#555">{h()!.tool_count} tools</text>
        </Show>
      </box>

      <box height={1} />
      <box width="100%" height={1} borderColor="#333" border={["top"]} />
      <box height={1} />

      {/* Sessions section */}
      <box flexDirection="column" flexGrow={1} paddingLeft={1} paddingRight={1}>
        <text fg="#555">Sessions</text>
        <box height={1} />
        <scrollbox flexGrow={1}>
          <For each={sessionList()}>
            {(s) => {
              const shortId = s.session_id.length > 8 ? s.session_id.slice(0, 8) : s.session_id;
              const isCurrent = s.session_id === props.currentSessionId;
              return (
                <box
                  height={1}
                  flexDirection="row"
                  backgroundColor={isCurrent ? "#3b82f644" : undefined}
                  on:press={() => props.onSelectSession(s.session_id)}
                >
                  <text fg={isCurrent ? "#3b82f6" : "#777"}>{isCurrent ? "► " : "  "}</text>
                  <text fg={isCurrent ? "#ccc" : "#555"}>{shortId}</text>
                  <box width={1} />
                  <text fg="#444">{s.model}</text>
                </box>
              );
            }}
          </For>
        </scrollbox>
      </box>

      {/* New session button */}
      <box height={1} />
      <box height={1} paddingLeft={1} paddingRight={1}>
        <box backgroundColor="#333" on:press={props.onNewSession}>
          <text fg="#ccc"> + New Session</text>
        </box>
      </box>
      <box height={1} />

      {/* Status */}
      <box height={1} paddingLeft={1} paddingRight={1} flexDirection="row">
        <text fg={h() ? "green" : "red"}>●</text>
        <text fg="#555"> {h() ? "connected" : "offline"}</text>
      </box>
      <box height={1} />
    </box>
  );
};
