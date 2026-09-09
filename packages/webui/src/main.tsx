import { render } from "solid-js/web";
import { RouterProvider, createRouter } from "@tanstack/solid-router";
import { routeTree } from "./routeTree.gen";
import { getRouter } from "./router";
import { getQueryClient } from "@repo/ui-solid/tanstack-query/provider";
import { QueryClientProvider } from "@tanstack/solid-query";
import { ThemeProvider } from "@repo/ui-solid/theme";

const router = getRouter();

const rootElement = document.getElementById("root")!;
if (!rootElement?.innerHTML) {
  render(
    () => (
      <QueryClientProvider client={getQueryClient()}>
        <ThemeProvider>
          <RouterProvider router={router} />
        </ThemeProvider>
      </QueryClientProvider>
    ),
    rootElement,
  );
}
