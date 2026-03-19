# Deploying codex-kanban docs on Vercel

## What Vercel is for in this project

`codex-kanban` is a local Rust CLI/TUI application.

Vercel should be used for the public-facing project website or documentation site, not for running the TUI runtime itself.

Use Vercel for:

- project landing page
- feature overview
- installation guide
- GIF or screenshot demos
- roadmap and changelog pages
- contributor and release documentation

Do not position Vercel as the host for the actual Codex terminal app.

## Recommended repository layout

The cleanest setup is to keep the application and docs site separate inside the same repository:

```text
codex-kanban/
├── codex-rs/           # Rust CLI/TUI source
├── docs/               # Markdown source docs
└── site/               # Future Next.js or static docs site for Vercel
```

If `site/` does not exist yet, create it later as a small standalone docs app. Keep it independent from the Rust workspace so the website can evolve without increasing the maintenance burden of the CLI.

## Recommended tech choice

For a low-maintenance docs site on Vercel:

- framework: Next.js
- package manager: `pnpm`
- root directory: `site`
- deployment goal: static or mostly-static marketing/docs pages

This aligns well with Vercel's default platform behavior and keeps deployment friction low.

## Suggested MVP pages

Start with a very small website:

1. Home
2. Why codex-kanban
3. Install from source
4. Key interactions: `/kb` and `~`
5. Screenshots or demo GIFs
6. Roadmap / product spec
7. Contributing

## Vercel project configuration

When the docs site exists, use these settings in Vercel:

- Framework Preset: `Next.js`
- Root Directory: `site`
- Install Command: `pnpm install`
- Build Command: `pnpm build`
- Output Directory: leave default for Next.js

If the site is fully static, you can also use:

- Build Command: `pnpm build`
- Output Directory: `out`

with `next export` style output if you choose that route.

## Environment variables

For a basic docs site, you usually need no secrets at all.

Only add environment variables if the website later includes:

- analytics
- feedback forms
- release feed integrations
- API-backed demos

Keep the docs site separate from any local Codex credentials.

## Deployment flow

1. Push `codex-kanban` to GitHub.
2. Open Vercel and import `duo121/codex-kanban`.
3. Select the future `site` directory as the root.
4. Confirm the framework and build settings.
5. Deploy the preview build.
6. Bind a custom domain if needed.

## Content strategy

The public site should explain the fork clearly:

- this is a fork of `openai/codex`
- the goal is multi-session board management with minimal upstream drift
- the official Codex interaction area is intentionally preserved
- the fork does not replace `/resume`; it adds `/kb` and board-local session management

That positioning matters because it helps users understand both the value of the fork and the maintenance philosophy.

## Recommended launch checklist

Before wiring Vercel:

1. Finalize the root `README.md`.
2. Add screenshots or short terminal demo recordings.
3. Publish the product spec and slash-command docs.
4. Create a minimal website in `site/`.
5. Connect the repo to Vercel.

## Current status

At the time of writing, this repository contains the Rust implementation and Markdown documentation, but not a dedicated Vercel-ready website directory yet.

That means the correct next step is:

1. keep the Rust/TUI code in this repository
2. add a lightweight `site/` docs app
3. deploy only that docs app to Vercel
