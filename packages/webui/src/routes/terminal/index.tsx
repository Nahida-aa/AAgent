import { createFileRoute } from "@tanstack/solid-router";
import { TerminalTabs } from "../../components/terminal-tabs";

export const Route = createFileRoute("/terminal/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <div class="h-screen flex flex-col">
      <header class="flex items-center justify-between px-4 py-2 border-b border-[#30363d] bg-[#161b22] shrink-0">
        <div class="flex items-center gap-3">
          <span class="font-semibold text-[#c9d1d9]">Terminal</span>
        </div>
        <a href="/" class="text-xs text-[#3b82f6] hover:text-[#60a5fa]">
          Chat
        </a>
      </header>
      <div class="flex-1 overflow-hidden">
        <TerminalTabs />
      </div>
    </div>
  );
}
