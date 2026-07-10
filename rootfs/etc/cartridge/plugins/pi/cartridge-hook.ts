import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { execSync } from "child_process";

function send(event: string, data: any) {
  try {
    execSync(`cartridge-api hook ${event}`, {
      input: JSON.stringify(data),
      timeout: 2000,
      stdio: ["pipe", "ignore", "ignore"],
    });
  } catch {}
}

export default function (pi: ExtensionAPI) {
  pi.on("tool_call", async (event) => {
    send("pi-tool-call", {
      tool: event.toolName,
      input: event.input,
    });
  });

  pi.on("tool_result", async (event) => {
    send("pi-tool-result", {
      tool: event.toolName,
      output: event.output,
    });
  });
}
