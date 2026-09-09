import { platform, release } from "os"
import { lazy } from "./lazy.js"
import { tmpdir } from "os"
import path from "path"
import fs from "fs/promises"

const writeWithStdin = async (cmd: string[], text: string): Promise<void> => {
  try {
    const proc = Bun.spawn(cmd, { stdin: "pipe" })
    proc.stdin.write(text)
    await proc.stdin.end()
    await proc.exited
  } catch {}
}

// Lazy load clipboardy to avoid expensive import at startup
const getClipboardy = lazy(async () => {
  const { default: clipboardy } = await import("clipboardy")
  return clipboardy
})

/**
 * Writes text to clipboard via OSC 52 escape sequence.
 * This allows clipboard operations to work over SSH by having
 * the terminal emulator handle the clipboard locally.
 */
function writeOsc52(text: string): void {
  if (!process.stdout.isTTY) return
  const base64 = Buffer.from(text).toString("base64")
  const osc52 = `\x1b]52;c;${base64}\x07`
  const passthrough = process.env["TMUX"] || process.env["STY"]
  const sequence = passthrough ? `\x1bPtmux;\x1b${osc52}\x1b\\` : osc52
  process.stdout.write(sequence)
}

export interface Content {
  data: string
  mime: string
}

// Checks clipboard for images first, then falls back to text.
export async function read(): Promise<Content | undefined> {
  const os = platform()

  if (os === "darwin") {
    const tmpfile = path.join(tmpdir(), "aagent-clipboard.png")
    try {
      const result = await Bun.$`osascript -e 'set imageData to the clipboard as "PNGf"' -e 'set fileRef to open for access POSIX file "${tmpfile}" with write permission' -e 'set eof fileRef to 0' -e 'write imageData to fileRef' -e 'close access fileRef'`.quiet()
      if (result.exitCode === 0) {
        const buffer = await fs.readFile(tmpfile)
        return { data: buffer.toString("base64"), mime: "image/png" }
      }
    } catch {
    } finally {
      await fs.rm(tmpfile, { force: true }).catch(() => {})
    }
  }

  // Windows/WSL: probe clipboard for images via PowerShell.
  if (os === "win32" || release().includes("WSL")) {
    const script =
      "Add-Type -AssemblyName System.Windows.Forms; $img = [System.Windows.Forms.Clipboard]::GetImage(); if ($img) { $ms = New-Object System.IO.MemoryStream; $img.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); [System.Convert]::ToBase64String($ms.ToArray()) }"
    try {
      const result = await Bun.$`powershell.exe -NonInteractive -NoProfile -command ${script}`.quiet()
      if (result.exitCode === 0) {
        const base64 = result.text().trim()
        if (base64) {
          const imageBuffer = Buffer.from(base64, "base64")
          if (imageBuffer.length > 0) {
            return { data: imageBuffer.toString("base64"), mime: "image/png" }
          }
        }
      }
    } catch {}
  }

  if (os === "linux") {
    try {
      const wayland = await Bun.$`wl-paste -t image/png`.quiet()
      if (wayland.exitCode === 0 && wayland.stdout.byteLength > 0) {
        return { data: Buffer.from(wayland.stdout).toString("base64"), mime: "image/png" }
      }
    } catch {}
    try {
      const x11 = await Bun.$`xclip -selection clipboard -t image/png -o`.quiet()
      if (x11.exitCode === 0 && x11.stdout.byteLength > 0) {
        return { data: Buffer.from(x11.stdout).toString("base64"), mime: "image/png" }
      }
    } catch {}
  }

  const clipboardy = await getClipboardy()
  const text = await clipboardy.read().catch(() => {})
  if (text) {
    return { data: text, mime: "text/plain" }
  }
}

const getCopyMethod = lazy(async () => {
  const os = platform()

  if (os === "darwin" && Bun.which("osascript")) {
    console.log("clipboard: using osascript")
    return async (text: string) => {
      const escaped = text.replace(/\\/g, "\\\\").replace(/"/g, '\\"')
      await writeWithStdin(["osascript", "-e", `set the clipboard to "${escaped}"`], text)
    }
  }

  if (os === "linux") {
    if (process.env["WAYLAND_DISPLAY"] && Bun.which("wl-copy")) {
      console.log("clipboard: using wl-copy")
      return (text: string) => writeWithStdin(["wl-copy"], text)
    }
    if (Bun.which("xclip")) {
      console.log("clipboard: using xclip")
      return (text: string) => writeWithStdin(["xclip", "-selection", "clipboard"], text)
    }
    if (Bun.which("xsel")) {
      console.log("clipboard: using xsel")
      return (text: string) => writeWithStdin(["xsel", "--clipboard", "--input"], text)
    }
  }

  if (os === "win32") {
    console.log("clipboard: using powershell")
    return (text: string) =>
      writeWithStdin(
        [
          "powershell.exe",
          "-NonInteractive",
          "-NoProfile",
          "-Command",
          "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; Set-Clipboard -Value ([Console]::In.ReadToEnd())",
        ],
        text,
      )
  }

  console.log("clipboard: no native support")
  return async (text: string) => {
    const clipboardy = await getClipboardy()
    await clipboardy.write(text).catch(() => {})
  }
})

export async function copy(text: string): Promise<void> {
  writeOsc52(text)
  const method = await getCopyMethod()
  await method(text)
}

export * as Clipboard from "./clipboard"
