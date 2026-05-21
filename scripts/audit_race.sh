#!/usr/bin/env bash
# Compare race/* branches against the sub-project #4 brief and emit a markdown scorecard.
#
# Usage:
#   ./scripts/audit_race.sh                  # audits all four race/* branches
#   ./scripts/audit_race.sh race/claude      # audits a specific branch
#
# Output: RACE_AUDIT.md at the repo root.
# Side effects: creates and removes git worktrees under .race-audit/ (gitignored if absent).
#
# Read-only — never modifies branches or pushes.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

DEFAULT_BRANCHES=(race/claude race/windsurf race/devin race/codex)
if [ "$#" -gt 0 ]; then
    BRANCHES=("$@")
else
    BRANCHES=("${DEFAULT_BRANCHES[@]}")
fi

WORKTREE_BASE="$REPO_ROOT/.race-audit"
mkdir -p "$WORKTREE_BASE"

OUT="$REPO_ROOT/RACE_AUDIT.md"

# Baseline test count from the brief (pre sub-project #4 work).
BASELINE_TESTS=88
BASELINE_RUST_UNIT=44
BASELINE_FRONTEND=34
BASELINE_RUST_INT=2
BASELINE_E2E=2
BASELINE_MOCK=6

# Expected files for plan fidelity (Rust agent module + frontend Fleet/LiveTape/AgentConfig).
EXPECTED_FILES=(
    "apps/desktop/src-tauri/src/agent/mod.rs"
    "apps/desktop/src-tauri/src/agent/agent.rs"
    "apps/desktop/src-tauri/src/agent/runtime.rs"
    "apps/desktop/src-tauri/src/agent/state.rs"
    "apps/desktop/src-tauri/src/agent/config.rs"
    "apps/desktop/src-tauri/src/agent/signal.rs"
    "apps/desktop/src-tauri/src/agent/executor.rs"
    "apps/desktop/src-tauri/src/agent/decision.rs"
    "apps/desktop/src-tauri/src/agent/llm/mod.rs"
    "apps/desktop/src-tauri/src/agent/llm/anthropic_direct.rs"
    "apps/desktop/src-tauri/src/agent/llm/mantic_proxy.rs"
    "apps/desktop/src-tauri/src/agent/llm/prompt.rs"
    "apps/desktop/src/routes/Fleet.tsx"
    "apps/desktop/src/components/LiveTape.tsx"
    "apps/desktop/src/routes/AgentConfig.tsx"
)

# Expected new Tauri commands (substring grep in commands.rs).
EXPECTED_COMMANDS=(
    "agent_create"
    "agent_list"
    "agent_get"
    "agent_arm"
    "agent_pause"
    "agent_kill"
    "agent_fire_test_signal"
    "agent_set_llm_key"
)

# --- helpers -----------------------------------------------------------------

log() { printf "\033[1;36m[audit]\033[0m %s\n" "$*" >&2; }
warn() { printf "\033[1;33m[audit]\033[0m %s\n" "$*" >&2; }
err() { printf "\033[1;31m[audit]\033[0m %s\n" "$*" >&2; }

# count_passing_tests <log-file> -- extracts "N passed" from cargo or vitest output
count_passing_tests() {
    grep -oE '[0-9]+ passed' "$1" 2>/dev/null | head -1 | awk '{print $1}' || echo 0
}

count_failing_tests() {
    grep -oE '[0-9]+ failed' "$1" 2>/dev/null | head -1 | awk '{print $1}' || echo 0
}

# --- main loop ---------------------------------------------------------------

# Print scorecard header.
{
    echo "# Race Audit — sub-project #4"
    echo
    echo "Generated: $(date -u +'%Y-%m-%d %H:%M:%S UTC')"
    echo
    echo "Baseline (before sub-project #4): $BASELINE_TESTS tests across all suites."
    echo
    echo "| Branch | Status | Commits | Files +/- | LOC + | Rust unit | Rust int | Frontend | Playwright | Plan files | Commands | Time-to-finish |"
    echo "|---|---|---|---|---|---|---|---|---|---|---|---|"
} > "$OUT"

# Per-branch details accumulator.
DETAILS=()

for branch in "${BRANCHES[@]}"; do
    log "auditing $branch"

    slug="$(echo "$branch" | sed 's|race/||')"
    wt="$WORKTREE_BASE/$slug"
    logdir="$WORKTREE_BASE/logs/$slug"
    mkdir -p "$logdir"

    # Reset worktree if it exists.
    if [ -d "$wt" ]; then
        git worktree remove --force "$wt" 2>/dev/null || rm -rf "$wt"
    fi

    # Try to materialise the branch.
    if ! git ls-remote --heads origin "$branch" | grep -q "$branch"; then
        warn "$branch does not exist on origin — skipping"
        echo "| \`$branch\` | ❌ NOT PUSHED | — | — | — | — | — | — | — | — | — | — |" >> "$OUT"
        continue
    fi

    git fetch -q origin "$branch:refs/remotes/origin/$branch" 2>/dev/null || true
    if ! git worktree add --detach "$wt" "origin/$branch" >"$logdir/worktree.log" 2>&1; then
        err "could not create worktree for $branch"
        echo "| \`$branch\` | ❌ WORKTREE FAIL | — | — | — | — | — | — | — | — | — | — |" >> "$OUT"
        continue
    fi

    # Commit + file metrics relative to main.
    cd "$wt"
    commits=$(git rev-list --count main..HEAD 2>/dev/null || echo 0)
    files_changed=$(git diff --name-only main...HEAD 2>/dev/null | wc -l | tr -d ' ')
    loc_added=$(git diff --shortstat main...HEAD 2>/dev/null | grep -oE '[0-9]+ insertion' | awk '{print $1}' || echo 0)
    [ -z "$loc_added" ] && loc_added=0

    if [ "$commits" -eq 0 ]; then
        warn "$branch has 0 commits ahead of main"
        echo "| \`$branch\` | ⏳ NO PROGRESS | 0 | 0 | 0 | — | — | — | — | — | — | — |" >> "$OUT"
        cd "$REPO_ROOT"
        git worktree remove --force "$wt" 2>/dev/null || true
        continue
    fi

    # Time-to-finish: first commit timestamp to last commit timestamp (UTC).
    first_ts=$(git log --reverse --format=%ct main..HEAD 2>/dev/null | head -1)
    last_ts=$(git log --format=%ct main..HEAD 2>/dev/null | head -1)
    if [ -n "$first_ts" ] && [ -n "$last_ts" ]; then
        delta=$(( last_ts - first_ts ))
        hours=$(( delta / 3600 ))
        mins=$(( (delta % 3600) / 60 ))
        ttf="${hours}h ${mins}m"
    else
        ttf="?"
    fi

    # Plan-file fidelity: how many of the expected files exist?
    fcount=0
    fmissing=()
    for f in "${EXPECTED_FILES[@]}"; do
        if [ -f "$f" ]; then
            fcount=$((fcount + 1))
        else
            fmissing+=("$f")
        fi
    done

    # Command fidelity: how many of the expected commands are mentioned in commands.rs?
    ccount=0
    cmissing=()
    if [ -f "apps/desktop/src-tauri/src/commands.rs" ]; then
        for c in "${EXPECTED_COMMANDS[@]}"; do
            if grep -q "fn ${c}\b\|fn ${c}(" apps/desktop/src-tauri/src/commands.rs; then
                ccount=$((ccount + 1))
            else
                cmissing+=("$c")
            fi
        done
    fi

    # ---- Rust unit tests ----
    log "  cargo test --lib (race/$slug)"
    rust_unit_passed=0
    rust_unit_failed=0
    rust_unit_status="?"
    if (cd apps/desktop/src-tauri && cargo test --lib --no-fail-fast 2>&1 | tee "$logdir/cargo_test_lib.log" | tail -50) >/dev/null; then
        rust_unit_passed=$(grep -hE "test result: ok\. [0-9]+ passed" "$logdir/cargo_test_lib.log" | awk '{sum += $4} END {print sum+0}')
        rust_unit_failed=$(grep -hE "test result: FAILED\. [0-9]+ passed; [0-9]+ failed" "$logdir/cargo_test_lib.log" | awk '{sum += $6} END {print sum+0}')
        if [ "$rust_unit_failed" -gt 0 ]; then
            rust_unit_status="❌ ${rust_unit_passed}p/${rust_unit_failed}f"
        else
            rust_unit_status="✅ $rust_unit_passed"
        fi
    else
        rust_unit_status="💥 build fail"
    fi

    # ---- Rust integration tests ----
    log "  cargo test --tests (with --features test-helpers for agent_loop)"
    rust_int_passed=0
    rust_int_failed=0
    rust_int_status="?"
    # Some branches may not have the test-helpers feature; try with, fall back without.
    if (cd apps/desktop/src-tauri && cargo test --tests --features test-helpers --no-fail-fast 2>&1 > "$logdir/cargo_test_tests.log") || \
       (cd apps/desktop/src-tauri && cargo test --tests --no-fail-fast 2>&1 > "$logdir/cargo_test_tests.log"); then
        rust_int_passed=$(grep -hE "test result: ok\. [0-9]+ passed" "$logdir/cargo_test_tests.log" | awk '{sum += $4} END {print sum+0}')
        rust_int_failed=$(grep -hE "test result: FAILED\. [0-9]+ passed; [0-9]+ failed" "$logdir/cargo_test_tests.log" | awk '{sum += $6} END {print sum+0}')
        if [ "$rust_int_failed" -gt 0 ]; then
            rust_int_status="❌ ${rust_int_passed}p/${rust_int_failed}f"
        else
            rust_int_status="✅ $rust_int_passed"
        fi
    else
        rust_int_status="💥 build fail"
    fi

    # ---- Frontend Vitest ----
    log "  pnpm install (race/$slug)"
    if (pnpm install --frozen-lockfile=false 2>&1 > "$logdir/pnpm_install.log"); then
        log "  pnpm --filter mantic-desktop test"
        if (pnpm --filter mantic-desktop test 2>&1 > "$logdir/vitest.log"); then
            frontend_passed=$(grep -oE 'Tests +[0-9]+ passed' "$logdir/vitest.log" | head -1 | awk '{print $2}')
            [ -z "$frontend_passed" ] && frontend_passed=0
            frontend_failed=$(grep -oE 'Tests +[0-9]+ failed' "$logdir/vitest.log" | head -1 | awk '{print $2}')
            [ -z "$frontend_failed" ] && frontend_failed=0
            if [ "$frontend_failed" -gt 0 ]; then
                frontend_status="❌ ${frontend_passed}p/${frontend_failed}f"
            else
                frontend_status="✅ $frontend_passed"
            fi
        else
            # vitest exits non-zero on test failure even without "Tests N failed"
            frontend_passed=$(grep -oE 'Tests +[0-9]+ passed' "$logdir/vitest.log" | head -1 | awk '{print $2}')
            [ -z "$frontend_passed" ] && frontend_passed=0
            frontend_failed=$(grep -oE 'Tests +[0-9]+ failed' "$logdir/vitest.log" | head -1 | awk '{print $2}')
            [ -z "$frontend_failed" ] && frontend_failed=0
            if [ "$frontend_failed" -gt 0 ]; then
                frontend_status="❌ ${frontend_passed}p/${frontend_failed}f"
            else
                frontend_status="💥 vitest fail"
            fi
        fi
    else
        frontend_status="💥 install fail"
    fi

    # ---- Playwright smoke ----
    log "  playwright (mantic-desktop-smoke)"
    playwright_status="—"
    if [ -d "apps/desktop-smoke" ]; then
        if (pnpm --filter mantic-desktop-smoke test 2>&1 > "$logdir/playwright.log"); then
            pw_pass=$(grep -oE '[0-9]+ passed' "$logdir/playwright.log" | head -1 | awk '{print $1}')
            [ -z "$pw_pass" ] && pw_pass=0
            pw_fail=$(grep -oE '[0-9]+ failed' "$logdir/playwright.log" | head -1 | awk '{print $1}')
            [ -z "$pw_fail" ] && pw_fail=0
            if [ "$pw_fail" -gt 0 ]; then
                playwright_status="❌ ${pw_pass}p/${pw_fail}f"
            else
                playwright_status="✅ $pw_pass"
            fi
        else
            playwright_status="💥 fail"
        fi
    else
        playwright_status="🚫 no smoke"
    fi

    # ---- Overall status ----
    overall="⏳"
    if [ "$rust_unit_failed" -gt 0 ] || [ "$rust_int_failed" -gt 0 ] || [ "$frontend_failed" -gt 0 ]; then
        overall="❌ TESTS FAILED"
    elif [ "$rust_unit_status" = "💥 build fail" ] || [ "$frontend_status" = "💥 install fail" ]; then
        overall="💥 BROKEN BUILD"
    elif [ "$rust_unit_passed" -ge $BASELINE_RUST_UNIT ] && [ "$frontend_passed" -ge $BASELINE_FRONTEND ]; then
        overall="✅ GREEN"
    else
        overall="⚠️  REGRESSION"
    fi

    plan_files="$fcount/${#EXPECTED_FILES[@]}"
    cmd_files="$ccount/${#EXPECTED_COMMANDS[@]}"

    echo "| \`$branch\` | $overall | $commits | $files_changed | $loc_added | $rust_unit_status | $rust_int_status | $frontend_status | $playwright_status | $plan_files | $cmd_files | $ttf |" >> "$OUT"

    # Per-branch details payload.
    {
        echo
        echo "## \`$branch\`"
        echo
        echo "- Commits ahead of main: $commits"
        echo "- Files touched: $files_changed"
        echo "- Lines added: $loc_added"
        echo "- Time-to-finish: $ttf"
        echo "- Rust unit tests: $rust_unit_status"
        echo "- Rust integration tests: $rust_int_status"
        echo "- Frontend Vitest: $frontend_status"
        echo "- Playwright smoke: $playwright_status"
        echo "- Plan files present: $plan_files"
        if [ "$fcount" -lt "${#EXPECTED_FILES[@]}" ]; then
            echo "  - Missing:"
            for m in "${fmissing[@]}"; do
                echo "    - \`$m\`"
            done
        fi
        echo "- Tauri commands wired: $cmd_files"
        if [ "$ccount" -lt "${#EXPECTED_COMMANDS[@]}" ]; then
            echo "  - Missing: ${cmissing[*]}"
        fi
        echo
        echo "### Commits"
        echo
        echo '```'
        git log --oneline main..HEAD 2>/dev/null | head -30
        echo '```'
    } >> "$REPO_ROOT/.race-audit/details_${slug}.md"

    DETAILS+=("$REPO_ROOT/.race-audit/details_${slug}.md")

    cd "$REPO_ROOT"
    git worktree remove --force "$wt" 2>/dev/null || rm -rf "$wt"
done

# Append per-branch details.
echo >> "$OUT"
echo "---" >> "$OUT"
for d in "${DETAILS[@]}"; do
    [ -f "$d" ] && cat "$d" >> "$OUT"
done

# Append criteria.
{
    echo
    echo "---"
    echo
    echo "## Criteria (from RACE_BRIEF.md)"
    echo
    echo "Win conditions, in priority order:"
    echo "1. All tests pass — Rust unit, frontend Vitest, Rust integration, Playwright smoke"
    echo "2. Plan fidelity — followed the spec'd file structure, types, commands"
    echo "3. No regression in existing tests — the $BASELINE_TESTS pre-existing tests must still pass"
    echo "4. Speed — first to satisfy 1–3 wins"
    echo "5. Code quality — tie-breaker; clean, idiomatic, no over-engineering"
    echo
    echo "Logs: \`$WORKTREE_BASE/logs/<branch-slug>/\`"
} >> "$OUT"

log "scorecard written to $OUT"
log "raw logs in $WORKTREE_BASE/logs/"
