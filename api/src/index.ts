import "dotenv/config";
import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { webhook } from "./webhook";

const app = new Hono();
app.route("/", webhook);
app.get("/health", (c) => c.text("ok"));

const port = parseInt(process.env.PORT ?? "3000");
serve({ fetch: app.fetch, port });
console.log(`Listening on :${port}`);
