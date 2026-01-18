# Task 02: Create Skills Directory Structure

## Metadata
- **Task ID**: TASK-GAP-003
- **Phase**: 1 (Foundation)
- **Priority**: High
- **Complexity**: Low
- **Estimated Time**: 15 minutes
- **Dependencies**: None

## Objective

Create the skills directory structure required by PRD Section 9.3. The PRD specifies 5 skills that must exist in `.claude/skills/*/SKILL.md` format. This task creates the directory structure that subsequent skill tasks (tasks 09-13) will populate with SKILL.md files.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/.claude/settings.json` - Verify .claude directory exists
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 3.1 for file structure specification

## Files to Create/Modify

**Directories to Create:**
- `/home/cabdru/contextgraph/.claude/skills/topic-explorer/`
- `/home/cabdru/contextgraph/.claude/skills/memory-inject/`
- `/home/cabdru/contextgraph/.claude/skills/semantic-search/`
- `/home/cabdru/contextgraph/.claude/skills/dream-consolidation/`
- `/home/cabdru/contextgraph/.claude/skills/curation/`

**Files to Create (placeholders for git tracking):**
- `/home/cabdru/contextgraph/.claude/skills/topic-explorer/.gitkeep`
- `/home/cabdru/contextgraph/.claude/skills/memory-inject/.gitkeep`
- `/home/cabdru/contextgraph/.claude/skills/semantic-search/.gitkeep`
- `/home/cabdru/contextgraph/.claude/skills/dream-consolidation/.gitkeep`
- `/home/cabdru/contextgraph/.claude/skills/curation/.gitkeep`

## Implementation Steps

### Step 1: Create the skills parent directory

```bash
mkdir -p /home/cabdru/contextgraph/.claude/skills
```

### Step 2: Create skill subdirectories with .gitkeep files

```bash
# topic-explorer
mkdir -p /home/cabdru/contextgraph/.claude/skills/topic-explorer
touch /home/cabdru/contextgraph/.claude/skills/topic-explorer/.gitkeep

# memory-inject
mkdir -p /home/cabdru/contextgraph/.claude/skills/memory-inject
touch /home/cabdru/contextgraph/.claude/skills/memory-inject/.gitkeep

# semantic-search
mkdir -p /home/cabdru/contextgraph/.claude/skills/semantic-search
touch /home/cabdru/contextgraph/.claude/skills/semantic-search/.gitkeep

# dream-consolidation
mkdir -p /home/cabdru/contextgraph/.claude/skills/dream-consolidation
touch /home/cabdru/contextgraph/.claude/skills/dream-consolidation/.gitkeep

# curation
mkdir -p /home/cabdru/contextgraph/.claude/skills/curation
touch /home/cabdru/contextgraph/.claude/skills/curation/.gitkeep
```

## Code/Content to Implement

### .gitkeep files

Each `.gitkeep` file should be empty (0 bytes). They exist solely to ensure git tracks the empty directories.

### Final Directory Structure

```
.claude/
  settings.json           # Already exists
  hooks/                  # Already exists
    session_start.sh
    session_end.sh
    pre_tool_use.sh
    post_tool_use.sh
    user_prompt_submit.sh
  skills/                 # NEW - created by this task
    topic-explorer/
      .gitkeep            # Placeholder until SKILL.md created in task09
    memory-inject/
      .gitkeep            # Placeholder until SKILL.md created in task10
    semantic-search/
      .gitkeep            # Placeholder until SKILL.md created in task11
    dream-consolidation/
      .gitkeep            # Placeholder until SKILL.md created in task12
    curation/
      .gitkeep            # Placeholder until SKILL.md created in task13
```

## Definition of Done

- [ ] Directory `/home/cabdru/contextgraph/.claude/skills/` exists
- [ ] Directory `/home/cabdru/contextgraph/.claude/skills/topic-explorer/` exists
- [ ] Directory `/home/cabdru/contextgraph/.claude/skills/memory-inject/` exists
- [ ] Directory `/home/cabdru/contextgraph/.claude/skills/semantic-search/` exists
- [ ] Directory `/home/cabdru/contextgraph/.claude/skills/dream-consolidation/` exists
- [ ] Directory `/home/cabdru/contextgraph/.claude/skills/curation/` exists
- [ ] All 5 directories contain `.gitkeep` files for git tracking
- [ ] Directory names are lowercase with hyphens (matching PRD Section 9.3 skill names)

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify all 5 skill directories exist
ls -la .claude/skills/
# Should show: topic-explorer, memory-inject, semantic-search, dream-consolidation, curation

# Verify each directory has .gitkeep
ls -la .claude/skills/topic-explorer/
ls -la .claude/skills/memory-inject/
ls -la .claude/skills/semantic-search/
ls -la .claude/skills/dream-consolidation/
ls -la .claude/skills/curation/

# Count directories (should be 5)
ls -d .claude/skills/*/ | wc -l
# Expected output: 5

# Verify directories are tracked by git
git status .claude/skills/
# Should show new untracked directories
```
