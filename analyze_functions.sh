#!/bin/bash
# analyze_functions.sh
# ChatGPT "Think" one-shot
# POSIX-minded Bash script (works on macOS default Bash 3.x).
# Usage:
#   chmod +x analyze_functions.sh
#   ./analyze_functions.sh                # uses default auth.rs path
#   ./analyze_functions.sh path/to/file.rs  # analyze another Rust file
#   cat functions.md                     # view results

set -u

# -------------------------
# Configurable service list
SERVICES=(
  "core/lending-service/src"
  "core/blockchain-monitor/src"
  "core/payment-channel-service/src"
)

INPUT_FILE=${1:-./core/common/src/auth.rs}

OUTPUT_FILE="functions.md"
TMP_HDRS="$(mktemp -t analyze_functions.hdr.XXXX 2>/dev/null || mktemp -t analyze_functions.hdr)" || { echo "Failed to create temp file"; exit 1; }

error_exit() {
  echo "Error: $1" >&2
  rm -f "$TMP_HDRS"
  exit 1
}

if [ ! -f "$INPUT_FILE" ]; then
  error_exit "Input file not found: $INPUT_FILE"
fi

ANY_SERVICE_OK=0
for svc in "${SERVICES[@]}"; do
  if [ -d "$svc" ]; then
    ANY_SERVICE_OK=1
    break
  fi
done
if [ "$ANY_SERVICE_OK" -eq 0 ]; then
  echo "Warning: none of the configured service directories exist. Check the SERVICES list near the top of the script." >&2
fi

# -------------------------------------------------------------------------
# Extract function headers using BSD awk-compatible code
awk '
BEGIN {
  in_block = 0
  prev_nonblank = ""
}

function trim(s) {
  gsub(/^[ \t\r\n]+|[ \t\r\n]+$/, "", s)
  return s
}

{
  line = $0

  if (in_block) {
    if (match(line, /\*\//)) {
      line = substr(line, RSTART + 2)
      in_block = 0
    } else {
      next
    }
  }

  # remove inline /* ... */ blocks
  while (match(line, /\/\*/)) {
    start = RSTART
    rest = substr(line, start + 2)
    if (match(rest, /\*\//)) {
      endpos = start + RSTART + 1
      line = substr(line, 1, start - 1) substr(line, endpos + 1)
    } else {
      line = substr(line, 1, start - 1)
      in_block = 1
      break
    }
  }

  # remove // comments
  if (match(line, /\/\//)) {
    line = substr(line, 1, RSTART - 1)
  }

  orig = line
  line = trim(line)

  if (line != "") {
    if (match(line, /^[[:space:]]*(pub([[:space:]]*\([^)]+\))?[[:space:]]*|pub[[:space:]]+|async[[:space:]]+|unsafe[[:space:]]+|extern[[:space:]]+|const[[:space:]]+)*fn[[:space:]]+/)) {
      header = orig

      # BSD-compatible parentheses counting
      lp = gsub("\\(", "", header)
      rp = gsub("\\)", "", header)
      paren_balance = lp - rp

      has_terminator = (match(header, /\{/) || match(header, /;$/))

      # BSD-compatible multi-line accumulation
      while (paren_balance > 0 || !has_terminator) {
        if ((getline nextline) <= 0) break
        line2 = nextline

        if (in_block) {
          if (match(line2, /\*\//)) {
            line2 = substr(line2, RSTART + 2)
            in_block = 0
          } else {
            continue
          }
        }

        while (match(line2, /\/\*/)) {
          s = RSTART
          r = substr(line2, s + 2)
          if (match(r, /\*\//)) {
            endpos2 = s + RSTART + 1
            line2 = substr(line2, 1, s - 1) substr(line2, endpos2 + 1)
          } else {
            line2 = substr(line2, 1, s - 1)
            in_block = 1
            break
          }
        }

        if (match(line2, /\/\//)) {
          line2 = substr(line2, 1, RSTART - 1)
        }

        header = header " " line2

        lp = gsub("\\(", "", line2)
        rp = gsub("\\)", "", line2)
        paren_balance += lp - rp

        has_terminator = (has_terminator || match(line2, /\{/) || match(line2, /;[[:space:]]*$/))
      }

      htrim = trim(header)
      ptrim = trim(prev_nonblank)
      print "FUNCHDR|" ptrim "|" htrim
      next
    }

    prev_nonblank = orig
  }
}
' "$INPUT_FILE" > "$TMP_HDRS"

# -------------------------------------------------------------------------
# The rest of your original script (parsing headers, scanning services, output)
# remains exactly the same. From here on, just paste your existing code.
# For brevity, it is not repeated in full here, but the only required change was the awk block above.
# -------------------------------------------------------------------------

# Prepare output file
cat > "$OUTPUT_FILE" <<EOF
# Functions extracted from \`$INPUT_FILE\`

EOF

FUNC_COUNT=0

while IFS= read -r rawline; do
  case "$rawline" in
    FUNCHDR\|*)
      rest="${rawline#FUNCHDR|}"
      prev_line="${rest%%|*}"
      header="${rest#*|}"
      ;;
    *)
      continue
      ;;
  esac

  prev_line=$(echo "$prev_line" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  header_line=$(echo "$header" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')

  case "$prev_line" in
    \#\[* ) continue ;;
  esac

  fn_part=$(echo "$header_line" | sed -E 's/^[[:space:]]*(pub([[:space:]]*\([^)]+\))?[[:space:]]*|pub[[:space:]]+|async[[:space:]]+|unsafe[[:space:]]+|extern[[:space:]]+|const[[:space:]]+)*//')
  fn_name=$(echo "$fn_part" | sed -E 's/^fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*.*/\1/')

  if [ -z "$fn_name" ] || [ "$fn_name" = "$fn_part" ]; then
    fn_name=$(echo "$header_line" | sed -E 's/.*[[:space:]]fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*/\1/')
    if [ -z "$fn_name" ]; then continue; fi
  fi

  case "$fn_name" in test_* ) continue ;; esac

  params=$(printf "%s\n" "$header_line" | sed -nE 's/^[^{;]*\((.*)\)[^{;]*([;{].*)?$/\1/p' || true)
  params=$(echo "$params" | tr '\n' ' ' | sed -E 's/^[[:space:]]*|[[:space:]]*$//g')
  ret=$(printf "%s\n" "$header_line" | sed -nE 's/.*->[[:space:]]*([^\\{;]*).*/\1/p' || true)
  ret=$(echo "$ret" | sed -E 's/[[:space:]]+where[[:space:]].*$//; s/\{.*$//; s/;[[:space:]]*$//')
  ret=$(echo "$ret" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  [ -z "$ret" ] && ret="()"

  if [ -z "$params" ]; then
    signature="\`$fn_name()\`"
  else
    params_clean=$(echo "$params" | sed -E 's/[[:space:]]+/ /g' | sed -E 's/^[[:space:]]*|[[:space:]]*$//g')
    signature="\`$fn_name($params_clean) -> $ret\`"
  fi

  location="\`$INPUT_FILE\`"
  tags="[COMMON]"
  called_by_lines=""

  for svcdir in "${SERVICES[@]}"; do
    [ ! -d "$svcdir" ] && continue
    service_root=$(dirname "$svcdir")
    service_name=$(basename "$service_root")
    found_file=""
    IFS=''
    find "$svcdir" -type f -name '*.rs' -print0 2>/dev/null | while IFS= read -r -d '' candidate; do
      if grep -E -q '(^|[^A-Za-z0-9_])'"$fn_name"'[[:space:]]*\(' "$candidate" 2>/dev/null; then
        printf '%s\0' "$candidate" >> "${TMP_HDRS}.matches.$$"
        break
      fi
    done
    if [ -f "${TMP_HDRS}.matches.$$" ]; then
      read -r -d '' found_file < "${TMP_HDRS}.matches.$$"
      rm -f "${TMP_HDRS}.matches.$$"
    else
      found_file=""
    fi
    if [ -n "$found_file" ]; then
      relpath="${found_file#"$service_root"/}"
      called_by_lines="${called_by_lines}\n  - ${service_name}: ${relpath}"
      tags="${tags} [SERVICE-SPECIFIC: ${service_name}]"
    fi
  done

  cat >> "$OUTPUT_FILE" <<ENTRY
- $signature
  **Location:** $location
  **Tag:** \`$tags\`
  **Description:** 
  **Dependencies:** 
  **Called-by:**$(printf "%b" "$called_by_lines")
  **Distinct-from:** 

ENTRY

  FUNC_COUNT=$((FUNC_COUNT + 1))
done < "$TMP_HDRS"

[ "$FUNC_COUNT" -eq 0 ] && echo "Warning: no functions parsed from $INPUT_FILE." >&2
echo "Found $FUNC_COUNT function(s). Wrote output to $OUTPUT_FILE."
rm -f "$TMP_HDRS"

cat <<USAGE

How to run:
  chmod +x analyze_functions.sh
  ./analyze_functions.sh                 # uses default: $INPUT_FILE
  ./analyze_functions.sh path/to/file.rs  # analyze another Rust file
  cat $OUTPUT_FILE                        # view results

USAGE

exit 0
