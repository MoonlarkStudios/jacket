export function validateTitle(title) {
  if (
    !/^(?:feat|fix|perf|refactor|docs|test|build|ci|chore|revert|style)(?:\([a-zA-Z0-9_./-]+\))?!?: \S.+$/.test(
      title,
    )
  ) {
    throw new Error(
      'Use a Conventional Commit PR title, such as "feat: add album search" or "fix(linux): preserve unfinished downloads". The squash commit must use this title.',
    );
  }
}

export async function checkTitle({ github, context, core }) {
  const number =
    context.payload.pull_request?.number ||
    Number(context.payload.inputs?.pr_number);
  if (!Number.isSafeInteger(number) || number <= 0)
    throw new Error('A pull request number is required');
  const { data: pr } = await github.rest.pulls.get({
    ...context.repo,
    pull_number: number,
  });
  validateTitle(pr.title);
  core.info(`Conventional Commit title accepted for PR #${number}`);
}
