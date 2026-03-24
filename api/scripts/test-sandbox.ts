import { runAgent } from "../src/sandbox";

await runAgent({
  repo: "demo-org/ingest-rs",
  issueNumber: 1,
});
