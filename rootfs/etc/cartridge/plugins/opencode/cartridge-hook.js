const { execSync } = require("child_process");

function send(event, data) {
  try {
    execSync(`cartridge hook ${event}`, {
      input: JSON.stringify(data),
      timeout: 2000,
      stdio: ["pipe", "ignore", "ignore"],
    });
  } catch {}
}

module.exports = async ({ client, $ }) => {
  return {
    "tool.execute.before": async (input) => {
      send("opencode-tool-call", {
        tool: input.tool,
        args: input.args,
      });
    },
    "tool.execute.after": async (input) => {
      send("opencode-tool-result", {
        tool: input.tool,
        result: input.result,
      });
    },
  };
};
