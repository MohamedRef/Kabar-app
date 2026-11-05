#!/bin/bash
# fixmin.sh [repo]: copy the validator's minute estimates into the curriculum yaml
cd ${1:-.}
bash "$(dirname "$(readlink -f "$0")")/vcheck.sh" . | grep "curriculum minutes must be" | while read l; do
  id=$(echo "$l" | sed -E 's|^(id\|jv)/([^ ]+)\.yaml.*|\2|'); want=$(echo "$l" | sed -E 's/.*estimate, ([0-9]+) .*/\1/')
  f=curriculum/indonesian.yaml; [[ $id == j* ]] && f=curriculum/javanese.yaml
  sed -i -E "/id: $id,/ s/minutes: [0-9]+/minutes: $want/" $f; echo "$id -> $want"
done
