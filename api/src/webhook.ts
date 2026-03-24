import { Hono } from "hono";
import { webhooks, postComment } from "./github";
import { runAgent } from "./sandbox";

const TRIGGER = "@myagent";

export const webhook = new Hono();

webhook.post("/webhooks/github", async (c) => {
  const body = await c.req.text();
  const sig = c.req.header("x-hub-signature-256") ?? "";

  if (!(await webhooks.verify(body, sig))) return c.text("bad signature", 401);

  const event = c.req.header("x-github-event");
  if (event !== "issue_comment") return c.text("ignored", 200);

  const payload = JSON.parse(body);
  if (payload.action !== "created") return c.text("ignored", 200);
  if (!payload.comment.body.includes(TRIGGER)) return c.text("ignored", 200);

  const ctx = {
    repo: payload.repository.full_name,
    issueNumber: payload.issue.number,
  };

  await postComment(ctx.repo, ctx.issueNumber, "⏳ Working on it — sandbox starting...");

  runAgent(ctx).catch((err) => {
    console.error("Agent failed:", err);
    postComment(ctx.repo, ctx.issueNumber, `❌ Agent failed: ${err.message}`).catch(() => {});
  });

  return c.text("ok", 200);
});
