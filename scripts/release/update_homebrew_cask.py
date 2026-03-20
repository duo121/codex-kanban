#!/usr/bin/env python3
"""Generate Homebrew cask metadata for codex-kanban from GitHub release assets."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from pathlib import Path

DEFAULT_REPO = "duo121/codex-kanban"
CASK_NAME = "codex-kanban"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--version",
        required=True,
        help="Release version (for example: 0.1.0 or rust-v0.1.0).",
    )
    parser.add_argument(
        "--repo",
        default=DEFAULT_REPO,
        help=f"GitHub repo in owner/name format (default: {DEFAULT_REPO}).",
    )
    parser.add_argument(
        "--tap-dir",
        type=Path,
        required=True,
        help="Path to the homebrew tap repository root.",
    )
    return parser.parse_args()


def normalize_version(raw: str) -> str:
    if raw.startswith("rust-v"):
        return raw.removeprefix("rust-v")
    if raw.startswith("v"):
        return raw.removeprefix("v")
    return raw


def release_tag(version: str) -> str:
    return f"rust-v{version}"


def download_asset(repo: str, tag: str, filename: str, output_dir: Path) -> Path:
    subprocess.run(
        [
            "gh",
            "release",
            "download",
            tag,
            "--repo",
            repo,
            "--pattern",
            filename,
            "--dir",
            str(output_dir),
        ],
        check=True,
    )
    path = output_dir / filename
    if not path.exists():
        raise FileNotFoundError(f"Expected release asset missing: {path}")
    return path


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def render_cask(version: str, repo: str, arm_sha256: str, intel_sha256: str) -> str:
    return f'''cask "{CASK_NAME}" do
  version "{version}"

  on_arm do
    sha256 "{arm_sha256}"
    url "https://github.com/{repo}/releases/download/rust-v#{{version}}/codex-npm-darwin-arm64-#{{version}}.tgz"

    binary "package/vendor/aarch64-apple-darwin/codex/codex", target: "codexkb"
    binary "package/vendor/aarch64-apple-darwin/codex/codex", target: "codex-kanban"
    binary "package/vendor/aarch64-apple-darwin/codex/codex", target: "codex-kanabn"
  end

  on_intel do
    sha256 "{intel_sha256}"
    url "https://github.com/{repo}/releases/download/rust-v#{{version}}/codex-npm-darwin-x64-#{{version}}.tgz"

    binary "package/vendor/x86_64-apple-darwin/codex/codex", target: "codexkb"
    binary "package/vendor/x86_64-apple-darwin/codex/codex", target: "codex-kanban"
    binary "package/vendor/x86_64-apple-darwin/codex/codex", target: "codex-kanabn"
  end

  name "codex-kanban"
  desc "Kanban-style multi-session workflow for OpenAI Codex CLI"
  homepage "https://github.com/{repo}"

  livecheck do
    url :url
    strategy :github_latest
  end
end
'''


def main() -> int:
    args = parse_args()
    version = normalize_version(args.version)
    tag = release_tag(version)

    tap_dir = args.tap_dir.resolve()
    cask_dir = tap_dir / "Casks"
    cask_dir.mkdir(parents=True, exist_ok=True)

    arm_filename = f"codex-npm-darwin-arm64-{version}.tgz"
    intel_filename = f"codex-npm-darwin-x64-{version}.tgz"

    with tempfile.TemporaryDirectory(prefix="codex-kanban-cask-") as tmp:
        tmp_dir = Path(tmp)
        arm_path = download_asset(args.repo, tag, arm_filename, tmp_dir)
        intel_path = download_asset(args.repo, tag, intel_filename, tmp_dir)
        arm_sha = sha256_of(arm_path)
        intel_sha = sha256_of(intel_path)

    cask_path = cask_dir / f"{CASK_NAME}.rb"
    cask_path.write_text(render_cask(version, args.repo, arm_sha, intel_sha), encoding="utf-8")
    print(f"Updated {cask_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
