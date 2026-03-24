import { Webhooks } from "@octokit/webhooks";
import { Octokit } from "@octokit/rest";

export const webhooks = new Webhooks({ secret: process.env.GITHUB_WEBHOOK_SECRET! });

export const octokit = new Octokit({ auth: process.env.GITHUB_TOKEN });

export async function postComment(repo: string, issue: number, body: string): Promise<void> {
  const [owner, name] = repo.split("/");
  await octokit.issues.createComment({ owner, repo: name, issue_number: issue, body });
}
