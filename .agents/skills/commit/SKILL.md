---
name: commit
description: Use this skill when asked to create or amend a commit.
---

# Commit

Use this skill whenever creating or amending a commit.

## 1) Fetch and follow official commit guidelines

Run:

```bash
scripts/fetch-commit-guidelines.sh
```

Use that output as the source of truth for commit format/rules.

**Exception:** Do not **manually wrap lines** or **enforce maximum line length**, ignore any instructions to the contrary.

## 2) Write the commit body for maintainers

Commit messages are reused as PR descriptions. Therefore, write commit messages keeping in mind that the primary audiences are human code reviewers and future maintainers. Optimize for skimmability while retaining sufficient context around changes, but do not repeat context that is easily inferred from the changes themselves, linked issues, or background information that mainters with at least a basic familiarity of the codebase would possess. 

Some tips:
- include brief context for why the change is needed
- include why this approach was chosen (when relevant)
- include links to relevant sources/issues/docs when useful
- be concise, human, and specific
- assume reviewers will skim the linked issue; do not restate it in depth

Commit messages use Markdown formatting. For example, use backticks for technical literals, inline links for URLs, and lists where useful.

When committing, you should use heredoc format to preserve newlines and other formatting.

## 3) Append Commit Footer

If a commit is related to a GitHub issue, this must be noted in a footer.

These footers must be placed on their own lines. The footer looks like the following:

```
[keyword] #[issue-id]
```

When the issue is in a different repo, use `[keyword] [repo]#[issue-id]` or, if the repo belongs to a different owner, `[keyword] [owner]/[repo]#[issue-id]`.

The keywords "Closes", "Fixes" and "Resolves" indicate that the commit fully addresses the issue. Merging a pull request containing such a commit will close the referenced issue.

The keywords "References", "Related to", and "Contributes to" may be used to indicate a relation to the issue, when the issue is not fully addressed by the commit. The issue will not be auto-closed upon merge.

One commit may contain zero or more footers; make sure all related issues you are aware of have a corresponding footer.

A pre-commit hook will take care of linking Linear issues, where applicable. Do not manually add these links, or use any format other than what is described here. You need to follow this precise format so that the pre-commit hook can work properly.
