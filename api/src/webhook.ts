import { Hono } from "hono";
import { webhooks, postComment } from "./github";

const TRIGGER = "@myagent";

const PLATFORM_API = process.env.PLATFORM_API_URL ?? "https://api.opencomputer.dev";
const PLATFORM_API_KEY = process.env.OPENCOMPUTER_API_KEY ?? "";

export const webhook = new Hono();

webhook.post("/webhooks/github", async (c) => {
  const body = await c.req.text();
  const sig = c.req.header("x-hub-signature-256") ?? "";

  try {
    if (!(await webhooks.verify(body, sig))) {
      console.error("Webhook signature mismatch");
      return c.text("bad signature", 401);
    }
  } catch (e: any) {
    console.error("Webhook verify error:", e.message);
    return c.text("bad signature", 401);
  }

  const event = c.req.header("x-github-event");
  if (event !== "issue_comment") return c.text("ignored", 200);

  const payload = JSON.parse(body);
  if (payload.action !== "created") return c.text("ignored", 200);
  if (!payload.comment.body.includes(TRIGGER)) return c.text("ignored", 200);

  const repo = payload.repository.full_name;
  const issueNumber = payload.issue.number;

  await postComment(repo, issueNumber, "⏳ Working on it — sandbox starting...");

  // Create session via platform API — fire and forget
  const res = await fetch(`${PLATFORM_API}/v1/agents/issue-resolver/sessions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-API-Key": PLATFORM_API_KEY,
    },
    body: JSON.stringify({
      input: {
        repo,
        issue_number: issueNumber,
        comment_author: payload.comment.user.login,
        comment_body: payload.comment.body,
      },
    }),
  });

  if (!res.ok) {
    const text = await res.text();
    console.error("Session creation failed:", res.status, text);
    await postComment(repo, issueNumber, `❌ Agent failed to start: ${res.status}`);
    return c.text("session creation failed", 500);
  }

  const session = await res.json();
  console.log(`Session ${session.id} created for ${repo}#${issueNumber}`);

  return c.text("ok", 200);
});
