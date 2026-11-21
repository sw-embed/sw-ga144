#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT
input="$scratch/source with spaces.f18"

golden() {
    local source=$1 expected='' pair address word actual
    shift
    printf '%s\n' "$source" > "$input"
    for pair in "$@"; do
        address=${pair%:*}
        word=${pair#*:}
        printf -v expected '%s%d %d\n' "$expected" "$address" "$((word))"
    done
    actual="$("$root/assemble" "$input")"
    if [[ $actual != "${expected%$'\n'}" ]]; then
        printf 'Source: %s\nExpected:\n%sActual:\n%s\n' "$source" "$expected" "$actual" >&2
        exit 1
    fi
}

reject() {
    local source=$1 message=$2 status=0
    printf '%s\n' "$source" > "$input"
    "$root/assemble" "$input" > "$scratch/stdout" 2> "$scratch/stderr" || status=$?
    if [[ $status != 1 || -s $scratch/stdout || $(<"$scratch/stderr") != "$message" ]]; then
        printf 'Expected exit 1 and diagnostic "%s" for source "%s". Got exit %d.\n' "$message" "$source" "$status" >&2
        cat "$scratch/stdout" "$scratch/stderr" >&2
        exit 1
    fi
}

# DB001 section 2.4.4: pack the raw slots, XOR the word, then insert
# the plain address field. Work the expected words from that encoding.
golden 'dup . + ;' 0:$(((24 << 13 | 28 << 8 | 20 << 3) ^ 0x15555))
golden 'dup dup dup a!' 0:$(((24 << 13 | 24 << 8 | 24 << 3 | 7) ^ 0x15555)) 1:$(((31 << 13) ^ 0x15555))
golden ': word dup ; 21 word ;' 0:0x25555 1:0x5700 2:21
golden 'dup dup # 0 call' 0:$(((((24 << 13 | 24 << 8 | 3 << 3) ^ 0x15555) & ~7)))
golden '# 7 org @p # xAB , dup # 0 jump' 7:$(((((8 << 13 | 24 << 8 | 28 << 3 | 7) ^ 0x15555)))) 8:0xAB 9:0x11400
golden '# x1ff jump' 0:0x115ff
golden '# 7 org dup dup # 0 jump' 7:$((((24 << 13 | 24 << 8 | 28 << 3 | 7) ^ 0x15555))) 8:0x11400
golden 'if 2 else 3 then ;' 0:0x19404 1:0x49b2 2:2 3:0x11406 4:0x49b2 5:3 6:0x15555
golden 'begin dup while drop repeat ;' 0:0x249b2 1:0x19403 2:0x3b700 3:0x15555
golden 'begin dup until ;' 0:0x25300 1:0x15555
golden '1 for dup unext ;' 0:0x48b2 1:1 2:0x25155
golden '1 for dup next ;' 0:0x48b2 1:1 2:0x25002 3:0x15555
golden 'leap then ;' 0:0x13401 1:0x15555
golden ': start @p # start , ;' 0:0x5555 1:0
golden $'( nested ( comments ) )\t# 62 org\r\n-x1 ;' 62:0x5555 63:0x3ffff
golden ''
golden '( comment )'

reject 'missing' 'error: unknown word missing'
reject 'unknown ; : unknown ;' 'error: unknown word unknown'
reject ': name ; : name ;' 'error: invalid or duplicate label name'
reject ': if' 'error: invalid or duplicate label if'
reject ':' 'error: missing label after :'
reject '# x64 org' 'error: org must advance to a RAM address (0..63)'
reject '# 63 org 1' 'error: image exceeds 64 RAM words'
reject '# x400 call' 'error: transfer address must be 0..1023'
reject '# nope ,' 'error: invalid host value nope'
reject 'if' 'error: unclosed control structure'
reject 'then' 'error: then needs a forward branch'
reject 'begin then' 'error: then needs a forward branch'
reject 'begin next' 'error: next or unext needs for'
reject '1 for dup dup dup dup unext ;' 'error: unext must fit within the first loop instruction word'
reject ')( ' 'error: unbalanced parenthesis comment'
reject '( missing close' 'error: unbalanced parenthesis comment'
printf 'F18 assembler gate passed.\n'

cargo test --offline --locked --quiet --manifest-path "$root/Cargo.toml"
simulate() {
    cargo run --offline --locked --quiet --manifest-path "$root/Cargo.toml" -- "$@"
}

"$root/assemble" "$root/examples/double.f18" > "$scratch/double.img"
actual="$(simulate 8 "000:1:$scratch/double.img")"
[[ $actual == '000 P=x000 slot=4 T=42 S=0 R=0 wait=-' ]]

"$root/assemble" "$root/examples/send.f18" > "$scratch/send.img"
"$root/assemble" "$root/examples/receive.f18" > "$scratch/receive.img"
actual="$(simulate 20 "000:0:$scratch/send.img" "001:0:$scratch/receive.img")"
[[ $actual == *'001 P=x002 slot=4 T=42 S=0 R=0 wait=-' ]]

printf '%s\n' ': entry 2 2 for 2* unext : done done ;' > "$input"
"$root/assemble" "$input" > "$scratch/loop.img"
actual="$(simulate 30 "000:0:$scratch/loop.img")"
[[ $actual == *' T=16 '* ]]

printf '%s\n' ': entry 0 if 2 else 3 then : done done ;' > "$input"
"$root/assemble" "$input" > "$scratch/branch.img"
actual="$(simulate 30 "000:0:$scratch/branch.img")"
[[ $actual == *' T=3 '* ]]

printf 'Assembler-to-emulator gate passed.\n'
