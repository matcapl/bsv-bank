#!/bin/bash

TIMESTAMP=$(date +"%Y%m%d_%H%M")
REPORT_FILE="./audit-reports/results_${TIMESTAMP}.txt"

# mkdir -p audit-reports

{
  ./stop-all.sh
  sleep 2
  ./scripts/stop-phase5-services.sh
  sleep 2
  ./start-all.sh
  sleep 5
  ./scripts/start-phase5-services.sh
  sleep 5
  ./scripts/check-phase5-status.sh
  sleep 5
  ./test-phase3-complete.sh
  ./test-phase4-complete.sh
  ./test-phase5-complete.sh
  ./test-phase6-complete-part1.sh
  ./test-phase6-complete-part2.sh
} | tee "$REPORT_FILE"
