import { ClientOnly, Link } from "@tanstack/solid-router";
import { Keyboard, Palette, Settings } from "lucide-solid";
import { Tabs, TabsContent, TabsIndicator, TabsList, TabsTrigger } from "@repo/ui-solid/base/tabs";
import { m } from "@repo/shared/i18n/paraglide/messages";
import { Modal } from "@repo/ui-solid/custom/modal/modal";
import type { JSX } from "solid-js";
import { openModal } from "@repo/ui-solid/custom/modal/renderer.tsx";
import { GeneralSettings } from "#/components/app/settings/general.tsx";

export const SettingsContent = () => {
  const baseItems = [
    {
      value: "general",
      label: m.general(),
      icon: Settings,
    },
    {
      value: "shortcuts",
      label: m.shortcuts(),
      icon: Keyboard,
    },
  ] as const;
  return (
    <ClientOnly>
      {/* <aside class="min-w-40 grid p-3 max-h-full h-full grid-rows-[1fr_auto] border-r border-border"> */}
      <Tabs defaultValue="general" orientation="vertical" class="gap-5">
        <TabsList class="mb-4" variant="side">
          {baseItems.map((item) => (
            <TabsTrigger value={item.value} class="gap-2">
              <item.icon size={16} /> {item.label}
            </TabsTrigger>
          ))}
        </TabsList>
        <TabsContent value="general">
          <GeneralSettings />
        </TabsContent>
        <TabsContent value="shortcuts">
          <h2>{m.shortcuts()}</h2>
        </TabsContent>
      </Tabs>
      {/* </aside> */}
      {/* <div class="h-full max-h-full min-h-0 pl-3 pt-3 grid grid-rows-[auto_1fr]">
				<h2 class="text-base font-medium mb-2 h-6">TitleString</h2>
				<div class={cn('min-h-0 overflow-y-auto ', scrollbarDefault)}>
				</div>
			</div> */}
    </ClientOnly>
  );
};
export const SettingsModal = (p: { children: JSX.Element }) => {
  return (
    <Modal Trigger={p.children} size="2xl">
      <SettingsContent />
    </Modal>
  );
};
export const openSettings = () => openModal(SettingsContent, { size: "4xl", class: "p-4" });
