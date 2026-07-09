import { execSync } from "child_process";

export default {
  tool_call: (event: any) => {
    try {
      execSync("cartridge-api hook pi-tool-call", {
        input: JSON.stringify(event),
        timeout: 5000,
      });
    } catch {}
  },
  tool_result: (event: any) => {
    try {
      execSync("cartridge-api hook pi-tool-result", {
        input: JSON.stringify(event),
        timeout: 5000,
      });
    } catch {}
  },
};
