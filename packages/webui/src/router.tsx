import { ErrorCard } from "@repo/ui-solid/app/error";
import { NotFound } from "@repo/ui-solid/app/NotFound";
import { getQueryClient } from "@repo/ui-solid/tanstack-query/provider";
import { createRouteMask, createRouter, RouterProvider } from "@tanstack/solid-router";
// Import the generated route tree
import { routeTree } from "./routeTree.gen";

// const settingsSubRoutes = ["/settings", "/settings/appearance"] as const;

// const settingsMasks = settingsSubRoutes.map((path) =>
// 	createRouteMask({
// 		routeTree,
// 		from: path,
// 		to: ".",
// 		search: true,
// 	}),
// );
export function getRouter() {
  // Create a new router instance
  const queryClient = getQueryClient();
  const router = createRouter({
    routeTree,
    context: { queryClient },
    scrollRestoration: true,
    defaultPreloadStaleTime: 0,
    // routeMasks: [...settingsMasks]
    defaultErrorComponent: ErrorCard,
    defaultNotFoundComponent: () => <NotFound />,
  });
  return router;
}

// Register the router instance for type safety
declare module "@tanstack/solid-router" {
  interface Register {
    router: ReturnType<typeof getRouter>;
  }
}
