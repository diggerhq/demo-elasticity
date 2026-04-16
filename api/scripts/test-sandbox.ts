import "dotenv/config";
import { runAgent } from "../src/sandbox";

await runAgent({
  repo: "diggerhq/demo-elasticity",
  issueNumber: 1,
});
