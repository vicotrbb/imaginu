# imaginu agent skill

A ready-to-install skill that teaches an AI coding agent to generate and iterate
real 3D assets with imaginu — write recipe → `generate --preview` → **look at
the PNG** → iterate. The skill is the *workflow*; `imaginu schema` is the
authoritative *reference* it points back to.

The skill lives in [`imaginu/SKILL.md`](imaginu/SKILL.md).

## Install for Claude Code

Copy the skill into your Claude skills directory (personal or project):

```sh
# Personal (available in every project):
cp -r skill/imaginu ~/.claude/skills/imaginu

# …or per-project (checked in with the repo):
mkdir -p .claude/skills && cp -r skill/imaginu .claude/skills/imaginu
```

Claude Code discovers it automatically from the YAML frontmatter — it triggers
when you ask for a 3D asset, character, or world. No further config needed.

## Install for Codex

Codex reads project instructions from `AGENTS.md` and custom prompts from
`~/.codex/prompts/`. Either works:

```sh
# As a reusable slash-prompt (personal):
mkdir -p ~/.codex/prompts && cp skill/imaginu/SKILL.md ~/.codex/prompts/imaginu.md

# …or append the workflow to your project's AGENTS.md so Codex always has it:
cat skill/imaginu/SKILL.md >> AGENTS.md
```

## Prerequisite

The agent needs the `imaginu` binary on `PATH`. The skill instructs it to check
`imaginu --version` and install via `install.sh` / `cargo binstall imaginu` if
missing, so you don't have to set that up in advance.

## Keeping it current

The skill deliberately avoids hardcoding recipe fields — it tells the agent to
run `imaginu schema` for the current contract. When imaginu adds new fields,
the skill keeps working without edits.
