import { HeadContent, Outlet, Scripts, createRootRoute } from '@tanstack/solid-router'
import { Suspense } from 'solid-js'
import styleCss from '../styles.css?url'
import { QueryClient  } from '@tanstack/solid-query';
import { getLocale } from '@repo/shared/i18n/utils';
import { isTauri } from '@tauri-apps/api/core';
import { invoke } from '@tauri-apps/api/core';
import { client } from '@aa/sdk';
import { themeScript } from '@repo/ui-solid/theme';
import { Devtools } from '@repo/ui-solid/app/devtools';
import { ModalRenderer } from '@repo/ui-solid/custom/modal/renderer';
import { Toaster } from '@repo/ui-solid/base/sonner';
import { SidebarProvider } from '@repo/ui-solid/base/sidebar';
import { AppSidebar } from '#/components/app/AppSidebar';

interface MyRouterContext {
  queryClient: QueryClient;
}
export const Route = createRootRoute<MyRouterContext>({
  head: () => ({
    title: 'AAgent',
    meta: [{
      name: 'viewport',
      content: 'width=device-width, initial-scale=1',
    }],
    links: [{ rel: 'stylesheet', href: styleCss }],
    scripts: [{ children: themeScript }],
	}),
  beforeLoad: async () => {
    if (typeof document !== 'undefined') {
			document.documentElement.setAttribute('lang', getLocale());
		}
    if (isTauri()) {
      const url = await invoke<string>('get_server_url');
      client.setConfig({ baseUrl: url });
    } else {
      document.documentElement.classList.add('browser')
      client.setConfig({ baseUrl: '/api' });
    }
  },
  shellComponent: RootComponent,
});

function RootComponent() {
  return (
    <>
      <HeadContent />
      <SidebarProvider>
        <AppSidebar />
        <div class="h-screen w-screen flex flex-col overflow-hidden bg-[#0d1117] text-[#c9d1d9]">
          <Suspense>
            <Outlet />
            <ModalRenderer />
            <Toaster duration={1000 * 10} />
          </Suspense>
        </div>
      </SidebarProvider>
      <Devtools />
      <Scripts />
    </>
  )
}
