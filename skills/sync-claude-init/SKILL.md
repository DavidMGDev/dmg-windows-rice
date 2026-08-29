---
name: sync-claude-init
description: "Push the skills in this rice repo's skills/ folder into the claude-init submodule so claude-init installs them on a new machine. Use after adding, renaming, editing or deleting a skill under QuickTools/skills/, or when claude-init ships a skill that no longer exists here."
disable-model-invocation: true
---

# Sync claude-init

`QuickTools/skills/` is the source of truth for skills I wrote.
`claude-init/skills/` is a **mirror** of it. This skill copies one to the
other and commits both repos.

Nothing else needs editing: `bin/claude-init.js` already discovers whatever is
in its `skills/` folder at runtime (any directory holding a `SKILL.md` that
starts with `---`). There is no list of skill names anywhere.

## What is not mirrored

- **This skill.** It only makes sense inside the rice repo, so it is excluded.
- **Plugins** (`mattpocock-skills`, `ponytail`, `impeccable`). Marketplaces,
  not files. claude-init installs them with `claude plugin install` and they
  update themselves. Copying their skills in here would just be a stale fork.
- **`no-ai-slop`.** Downloaded fresh from upstream on every claude-init run.

Those live in the `PLUGINS` / `REMOTE_SKILLS` lists at the top of
`bin/claude-init.js`. Edit those by hand; only the local folder is synced.

A plugin entry can carry a `setup` slash command, which claude-init runs in
Claude Code after installing. That is a property of the entry, not of this
sync, so adding a skill here never changes what setup runs.

## Steps

Run from the rice repo root.

**1. Dry run first.** `/MIR` deletes anything in the destination that is not in
the source, so look before you leap:

```powershell
robocopy skills claude-init\skills /MIR /XD sync-claude-init /NJH /NJS /NP /L
```

Read the output. `*EXTRA Dir` or `*EXTRA File` means the mirror is about to
delete something. If that is a skill someone added to `claude-init/skills/`
directly, move it into `skills/` here first, otherwise it is gone.

**2. Mirror.**

```powershell
robocopy skills claude-init\skills /MIR /XD sync-claude-init /NJH /NJS /NP
```

Robocopy exit codes below 8 mean success, so ignore a non-zero exit; only 8+
is a real failure.

**3. Update the bundled-skills table** in `claude-init/README.md`, under
`### Bundled skills`. It is written by hand and goes stale silently. One row
per folder now in `claude-init/skills/`, with the link and a one-line summary
taken from the skill's own `description`.

**4. Commit the submodule, then the pointer.**

```powershell
cd claude-init
git add -A
git commit -m "Sync skills from dmg-windows-rice"
git push
cd ..
git add -A
git commit -m "Sync claude-init skills"
git push
```

The second commit is what moves the submodule pointer. Skipping it leaves the
rice repo pointing at the old claude-init commit, so a fresh
`git clone --recurse-submodules` gets the stale skills.

## Checking it worked

```powershell
node claude-init\bin\claude-init.js --no-open
```

Every folder in `skills/` except `sync-claude-init` should print as
`(bundled, installed)` or `(bundled, updated)`. It writes to
`~/.claude/skills/`, so this both verifies the sync and installs the skills on
this machine.

## Adding a skill later

Drop a folder with a `SKILL.md` into `skills/`, run this skill. That is the
whole workflow — there is no registration step in either repo.
