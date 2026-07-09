const { execSync } = require("child_process");

module.exports = {
  onToolCall(event) {
    try {
      execSync("cartridge-api hook opencode-tool-call", {
        input: JSON.stringify(event),
        timeout: 5000,
      });
    } catch {}
  },
  onToolResult(event) {
    try {
      execSync("cartridge-api hook opencode-tool-result", {
        input: JSON.stringify(event),
        timeout: 5000,
      });
    } catch {}
  },
};
