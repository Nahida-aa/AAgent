import { Link } from "@tanstack/solid-router";
import packageJson from "../../../package.json";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@repo/ui-solid/base/sidebar";
import { openSettings, SettingsModal } from "./settings/settings";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@repo/ui-solid/base/dialog";
import { Button } from "@repo/ui-solid/base/button";
import { TextField, TextFieldInput, TextFieldLabel } from "@repo/ui-solid/base/text-field";
import { Settings } from "lucide-solid";
import { TooltipX } from "@repo/ui-solid/custom/tooltip";

export function AppSidebar() {
  return (
    <Sidebar>
      <SidebarHeader class="flex-row">
        <TooltipX content={`Version ${packageJson.version}`}>
          <Link to="/">
            <h1 class="flex gap-1">
              <span>aa</span>
              <span class="text-muted-foreground">bot</span>
            </h1>
          </Link>
        </TooltipX>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup></SidebarGroup>
        <SidebarGroup />
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenuButton onClick={() => openSettings()}>
          <Settings />
        </SidebarMenuButton>
        {/* <DialogDemo /> */}
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
