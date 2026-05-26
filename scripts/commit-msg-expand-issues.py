#!/usr/bin/env python3
"""Expand GitHub issue commit footers and add Linear footers when available."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

FOOTER_RE = re.compile(
    r"^(?P<prefix>\s*)(?P<keyword>\w+(?:\s+to)?)\s+"
    r"(?P<display>(?:(?P<owner>[A-Za-z0-9_.-]+)/)?(?:(?P<repo>[A-Za-z0-9_.-]+))?#(?P<issue>[1-9]\d*))"
    r"(?P<suffix>\.?\s*)$"
)
LINEAR_LINKBACK_AUTHORS = {"linear", "linear-code"}
LINEAR_LINKBACK_MARKERS = ("linear-linkback", "linear linkback")

LINEAR_URL_RE = re.compile(
    r"(?P<url>https://linear\.app/[^\s<>)\]\"']*/issue/(?P<id>[^/\s<>)\]\"']+)[^\s<>)\]\"']*)"
)


@dataclass(frozen=True)
class Match:
    line_index: int
    prefix: str
    keyword: str
    display: str
    owner: str | None
    repo: str | None
    issue: str
    suffix: str


@dataclass(frozen=True)
class IssueInfo:
    url: str
    linear_id: str | None = None
    linear_url: str | None = None


def warn(message: str) -> None:
    print(f"commit-msg-expand-issues: warning: {message}", file=sys.stderr)


def run_gh(args: list[str]) -> tuple[dict[str, Any] | None, str | None]:
    try:
        result = subprocess.run(
            ["gh", *args],
            check=False,
            capture_output=True,
            encoding="utf-8",
        )
    except FileNotFoundError:
        return None, "gh was not found"
    except OSError as exc:
        return None, f"failed to run gh: {exc}"

    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        return None, detail or f"gh exited with status {result.returncode}"

    try:
        return json.loads(result.stdout), None
    except json.JSONDecodeError as exc:
        return None, f"failed to parse gh output: {exc}"


def run_gh_text(args: list[str]) -> tuple[str | None, str | None]:
    try:
        result = subprocess.run(
            ["gh", *args],
            check=False,
            capture_output=True,
            encoding="utf-8",
        )
    except FileNotFoundError:
        return None, "gh was not found"
    except OSError as exc:
        return None, f"failed to run gh: {exc}"

    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        return None, detail or f"gh exited with status {result.returncode}"
    return result.stdout.strip(), None


def current_repo() -> tuple[str, str] | None:
    name_with_owner, error = run_gh_text(
        ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"]
    )
    if error is not None:
        warn(f"could not resolve current repository: {error}")
        return None

    if not name_with_owner or "/" not in name_with_owner:
        warn("could not resolve current repository: unexpected gh output")
        return None

    owner, repo = name_with_owner.split("/", 1)
    return owner, repo


def is_linear_linkback_comment(comment: dict[str, Any]) -> bool:
    author = comment.get("author")
    if (
        not isinstance(author, dict)
        or author.get("login") not in LINEAR_LINKBACK_AUTHORS
    ):
        return False

    body = comment.get("body")
    if not isinstance(body, str):
        return False

    normalized_body = body.lower()
    return any(marker in normalized_body for marker in LINEAR_LINKBACK_MARKERS)


def find_linear_link(issue: dict[str, Any]) -> tuple[str, str] | None:
    comments = issue.get("comments") or []
    if not isinstance(comments, list):
        return None

    for comment in comments:
        if not isinstance(comment, dict) or not is_linear_linkback_comment(comment):
            continue

        body = comment["body"]
        url_match = LINEAR_URL_RE.search(body)
        if not url_match:
            continue

        return url_match.group("id"), url_match.group("url")
    return None


def fetch_issue(owner_repo: str, issue_number: str) -> IssueInfo | None:
    result, error = run_gh(
        [
            "issue",
            "view",
            issue_number,
            "-R",
            owner_repo,
            "--json",
            "number,url,comments",
        ]
    )
    if error is not None:
        warn(f"could not fetch {owner_repo}#{issue_number}: {error}")
        return None
    if not isinstance(result, dict) or not isinstance(result.get("url"), str):
        warn(f"could not fetch {owner_repo}#{issue_number}: unexpected gh output")
        return None

    linear = find_linear_link(result)
    if linear is None:
        return IssueInfo(url=result["url"])
    linear_id, linear_url = linear
    return IssueInfo(url=result["url"], linear_id=linear_id, linear_url=linear_url)


def collect_matches(lines: list[str]) -> list[Match]:
    matches: list[Match] = []
    for index, line in enumerate(lines):
        stripped_newline = line.removesuffix("\n")
        match = FOOTER_RE.match(stripped_newline)
        if match is None:
            continue
        matches.append(
            Match(
                line_index=index,
                prefix=match.group("prefix"),
                keyword=match.group("keyword"),
                display=match.group("display"),
                owner=match.group("owner"),
                repo=match.group("repo"),
                issue=match.group("issue"),
                suffix=match.group("suffix"),
            )
        )
    return matches


def resolve_owner_repo(match: Match, current_owner: str, current_repo_name: str) -> str:
    if match.owner is not None and match.repo is not None:
        return f"{match.owner}/{match.repo}"
    if match.repo is not None:
        return f"{current_owner}/{match.repo}"
    return f"{current_owner}/{current_repo_name}"


def process_message(path: Path) -> None:
    try:
        lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    except OSError as exc:
        warn(f"could not read commit message: {exc}")
        return

    matches = collect_matches(lines)
    if not matches:
        return

    repo = current_repo()
    if repo is None:
        return
    current_owner, current_repo_name = repo

    issue_cache: dict[tuple[str, str], IssueInfo | None] = {}
    replacements: dict[int, str] = {}

    for match in matches:
        owner_repo = resolve_owner_repo(match, current_owner, current_repo_name)
        key = (owner_repo, match.issue)
        if key not in issue_cache:
            issue_cache[key] = fetch_issue(owner_repo, match.issue)

        issue = issue_cache[key]
        if issue is None:
            continue

        replacement = f"{match.prefix}{match.keyword} [{match.display}]({issue.url}){match.suffix}\n"
        if issue.linear_id is not None and issue.linear_url is not None:
            next_line = (
                lines[match.line_index + 1] if match.line_index + 1 < len(lines) else ""
            )
            linear_line = f"{match.prefix}{match.keyword} [{issue.linear_id}]({issue.linear_url})\n"
            if next_line != linear_line:
                replacement += linear_line
        replacements[match.line_index] = replacement

    if not replacements:
        return

    new_lines = [replacements.get(index, line) for index, line in enumerate(lines)]
    try:
        path.write_text("".join(new_lines), encoding="utf-8")
    except OSError as exc:
        warn(f"could not write commit message: {exc}")


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        warn("expected exactly one commit message file path")
        return 0

    process_message(Path(argv[1]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
