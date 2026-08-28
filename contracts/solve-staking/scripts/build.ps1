param(
    # cargo-build-sbf defaults to v0, and an Agave 4.2.1 validator refuses to
    # deploy a v0 executable: "Detected sbpf_version required by the executable
    # which are not enabled". Discovered on a local validator 2026-08-24, and it
    # would have shown up as a failed mainnet deploy otherwise. Pass the target
    # cluster's version explicitly rather than inheriting a default that no
    # longer matches any cluster.
    [ValidateSet("v0", "v1", "v2", "v3", "v4")]
    [string]$Arch = "v3"
)
$ErrorActionPreference = "Stop"
$project = (Resolve-Path "$PSScriptRoot\..").Path
$image = "solve-staking-build:latest"
# The program keypair only names the address a deploy publishes to; the
# bytecode does not depend on it in any way. Requiring it here made the build
# impossible for the one audience that matters most — someone checking the
# published hash against this source, who will never have our private key.
# Missing is therefore a note, not a failure. Deploying without it is the case
# that must not pass silently, and `solana program deploy --program-id` is
# where that is caught.
$programKey = Join-Path $project ".keys\solve_staking-keypair.json"
$deployDir = Join-Path $project "target\deploy"
New-Item -ItemType Directory -Force $deployDir | Out-Null
if (Test-Path $programKey) {
    Copy-Item -LiteralPath $programKey -Destination (Join-Path $deployDir "solve_staking-keypair.json") -Force
} else {
    Write-Host "no .keys/solve_staking-keypair.json - building anyway, the bytecode does not depend on it (it is needed only to deploy)"
}

docker build -f "$project\Dockerfile.build" -t $image $project
# No --features: a deployable binary must carry the real funding authority.
Write-Host "building for sbpf $Arch"
docker run --rm `
    -v "${project}:/work" `
    -v "solve-staking-sbf-cache:/root/.cache/solana" `
    $image cargo-build-sbf --manifest-path /work/Cargo.toml --arch $Arch

$program = Join-Path $project "target\deploy\solve_staking.so"
if (-not (Test-Path $program)) { throw "Build finished without $program" }

# Every feature that weakens the program for testing announces itself with a
# BUILT-WITH-* marker on initialize: test-authority swaps in a publicly known
# signing key, fast-clock shortens the hold to minutes. A binary carrying
# either must never reach a cluster, so refuse to hand one over.
$marker = "BUILT-WITH-"
$found = docker run --rm -v "${project}:/work" $image `
    sh -c "strings /work/target/deploy/solve_staking.so | grep -c '$marker' || true"
if ($found.Trim() -ne "0") {
    Remove-Item -LiteralPath $program -Force
    throw "Binary carries a BUILT-WITH-* testing marker. Deleted it; rebuild without features."
}

# The marker check above is negative: it proves the binary is missing something
# bad, and it depends on a `msg!` literal surviving. Nothing proved the binary
# contains the right things, so a typo in either compiled-in address would have
# travelled the whole pipeline and surfaced as a program pointed at the wrong
# mint, or at an authority key nobody holds.
#
# The obvious check — grepping `strings` for the base58 address — does not work
# and was tried: `pubkey!` is const-evaluated, so an address exists in the
# binary as 32 raw bytes and never as text. Verified on the 2026-08-26 build,
# where every address appears exactly once as bytes and zero times as text.
# So decode the base58 here and look for the bytes.
function ConvertFrom-Base58 {
    param([string]$Text)
    $alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
    $value = [bigint]::Zero
    foreach ($ch in $Text.ToCharArray()) {
        $index = $alphabet.IndexOf($ch)
        if ($index -lt 0) { throw "not base58: $Text" }
        $value = $value * 58 + $index
    }
    $bytes = New-Object System.Collections.Generic.List[byte]
    while ($value -gt 0) {
        $bytes.Insert(0, [byte]($value % 256))
        $value = [bigint]::Divide($value, 256)
    }
    foreach ($ch in $Text.ToCharArray()) {
        if ($ch -ne '1') { break }
        $bytes.Insert(0, [byte]0)
    }
    return , $bytes.ToArray()
}

function Test-ByteSequence {
    param([byte[]]$Haystack, [byte[]]$Needle)
    $limit = $Haystack.Length - $Needle.Length
    $first = $Needle[0]
    for ($i = 0; $i -le $limit; $i++) {
        if ($Haystack[$i] -ne $first) { continue }
        $match = $true
        for ($j = 1; $j -lt $Needle.Length; $j++) {
            if ($Haystack[$i + $j] -ne $Needle[$j]) { $match = $false; break }
        }
        if ($match) { return $true }
    }
    return $false
}

$image_bytes = [System.IO.File]::ReadAllBytes($program)

# Must be present: the addresses a deployable binary is meaningless without.
$required = [ordered]@{
    "mint"              = "GwyWFsDKW9a2ref1EWqdUS7B37Toii433zrAh9Dipump"
    "funding authority" = "TUgoFCpHXaNCpQFjPQwwULqh9AuTDEhbq7Z7fw3r971"
    "token program"     = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
}
foreach ($name in $required.Keys) {
    $address = $required[$name]
    if (-not (Test-ByteSequence -Haystack $image_bytes -Needle (ConvertFrom-Base58 $address))) {
        Remove-Item -LiteralPath $program -Force
        throw "Binary does not contain the expected $name $address. Deleted it."
    }
    Write-Host "contains $name : $address"
}

# Must be absent. This catches a testing build even if its BUILT-WITH marker
# were ever optimised away, because it looks for the substituted address itself
# rather than for a log line that mentions it.
$forbidden = [ordered]@{
    "test funding authority" = "7BRPh4s6sva7zH4sRHxNvf2cmtFLjoiVuBjgNvf2xFrr"
    "devnet mint"            = "9dybdAGgG1w4yZS4oBzgY424pxHscQFQkWr9qobRQvFH"
    "retired dev authority"  = "ACbZ5vajyFFseZoYTdrzcxLSJbnnf4pt3MQoZ7XtDrws"
}
foreach ($name in $forbidden.Keys) {
    $address = $forbidden[$name]
    if (Test-ByteSequence -Haystack $image_bytes -Needle (ConvertFrom-Base58 $address)) {
        Remove-Item -LiteralPath $program -Force
        throw "Binary carries the $name $address. Deleted it; rebuild without features."
    }
}
Write-Host "carries neither the test authority nor the devnet mint"

$bytes = (Get-Item $program).Length
Write-Host "program size : $bytes bytes"
Write-Host "sha256       : $((Get-FileHash -Algorithm SHA256 $program).Hash.ToLower())"
$programDataBytes = $bytes + 45
Write-Host "program data : $programDataBytes bytes (binary + loader header)"
# Publish this hash with every deployment. For an upgradeable program it is the
# only way an outsider can tie the bytecode on chain back to this source.
Write-Host "Record the sha256 above in docs/STAKING_FOR_HOLDERS.md before deploying."
docker run --rm $image solana rent $programDataBytes
