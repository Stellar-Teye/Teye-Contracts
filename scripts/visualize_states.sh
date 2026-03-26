#!/usr/bin/env bash
set -euo pipefail

out="${1:-state_machines.dot}"

cat > "$out" <<'DOT'
digraph state_machines {
  rankdir=LR;
  subgraph cluster_vision {
    label="VisionRecord Lifecycle";
    Draft -> PendingReview;
    PendingReview -> Approved;
    Approved -> Archived;
    Archived -> Purged;
  }
  subgraph cluster_prescription {
    label="Prescription Lifecycle";
    Created -> Dispensed;
    Dispensed -> PartiallyFilled;
    Dispensed -> Completed;
    PartiallyFilled -> Completed;
    Created -> Expired;
    Dispensed -> Expired;
    PartiallyFilled -> Expired;
  }
}
DOT

echo "Wrote $out"
