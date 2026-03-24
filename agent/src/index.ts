import { query } from "@anthropic-ai/claude-agent-sdk";
import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));

const { values } = parseArgs({
  options: {
    repo:  { type: "string" },
    issue: { type: "string" },
  },
  strict: true,
});

if (!values.repo || !values.issue) {
  console.error("Usage: index.ts --repo owner/repo --issue 42");
  process.exit(1);
}

const systemPrompt = readFileSync(join(__dirname, "../prompt.md"), "utf-8");

const stream = query({
  prompt: [
    `Resolve this GitHub issue.`,
    ``,
    `Repository: ${values.repo}`,
    `Issue number: ${values.issue}`,
    ``,
    `Start by running: gh issue view ${values.issue} --repo ${values.repo}`,
  ].join("\n"),
  options: {
    model: "claude-sonnet-4-6",
    systemPrompt,
    tools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    allowedTools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
    cwd: process.env.AGENT_WORKDIR ?? process.cwd(),
    maxTurns: 50,
  },
});

let exitCode = 0;

for await (const message of stream) {
  if (message.type === "assistant" && message.message?.content) {
    for (const block of message.message.content) {
      if (block.type === "text") {
        console.log("[agent]", block.text?.slice(0, 200));
      }
    }
  }

  if (message.type === "result") {
    if (message.subtype === "success") {
      console.log("Agent completed successfully.");
    } else {
      const errors = "errors" in message ? (message as any).errors : [];
      console.error("Agent failed:", message.subtype, errors);
      exitCode = 1;
    }
  }
}

process.exit(exitCode);
