import { tui, createTuiRenderer } from "./app"
import { ensureServer } from "./util/server"

const serverUrl = process.env.AA_SERVER_URL ?? "http://localhost:3000"
const stopServer = await ensureServer(serverUrl)

process.stdout.write("\x1b]0;AAgent\x07")

const renderer = await createTuiRenderer()
const handle = tui({
  url: serverUrl,
  args: {},
  renderer,
})

await handle.done
stopServer()
