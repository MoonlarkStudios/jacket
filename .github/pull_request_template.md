Use a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/) title, such as `feat: add album search` or `fix(linux): preserve unfinished jobs`. The PR title becomes the squash commit.

## Change

Describe the problem and resulting behavior. Link an issue if relevant.

## Validation

Describe the checks you ran and any limitations. Include screenshots for UI changes.

- [ ] `mise run check`
- [ ] `mise run test:media` for media, tagging, codec, or archive changes
- [ ] Relevant docs updated

Remove private URLs, account details, cookies, and local paths from evidence.
