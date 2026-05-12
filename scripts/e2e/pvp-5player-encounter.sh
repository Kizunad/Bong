#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"

cd "$REPO_ROOT/server"
cargo test pvp_five_player_encounter_matrix_emits_trackable_story
