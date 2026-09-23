# M9 gate: the Tauri shell, the typed bridge and the IPC benchmark (docs/rebuild-plan.md, M9).
#   (a) tauri >= 2.11.6 (cargo tree); the only Tauri plugin is dialog
#   (b) npx tauri build --no-bundle exits 0 (this also builds the UI: tsc + vite)
#   (c) app.exe --selftest: webgl.renderer contains NVIDIA on Grace; ipc.single_16MB_ms and
#       ipc.all_bands_98MB_ms present and recorded in docs/decisions/ipc.md; the 500 ms rule
#   (d) the panic probe comes back to the UI as an error, and a ping afterwards succeeds
#   (e) the TypeScript bindings regenerate to the same git blobs, and git diff is clean
#   (f) capabilities: 0 'shell:' and 0 'fs:' in every place Tauri reads a capability from; the
#       built dist: 0 googleapis and 0 http(s):// URLs outside the listed exceptions
#   (g) 0 non-async #[tauri::command]
#   (h) the step bar shows the five steps, read from the DOM by the app's own self-test
# Plus: every command body runs through the panic guard, strict CSP, WebView2-specific code only
# in webview2.rs, run events batched at 50 ms (measured), the checksum known answers, the app
# crate's tests, clippy -D warnings and rustfmt for the app crate, and the UI typecheck.
#
# Grace-local (plan, critic flaw 4): (c) needs the NVIDIA GPU and opens a window.
# Run: powershell -File tools/gates/m9.ps1 [-TargetDir <dir>] [-Jobs <n>]
# Partial runs, for checking the gate itself (they never print "M9 PASSED"):
#   -Only static     the text checks only: (g), the guard, (f) capabilities, CSP, WebView2
#   -Only bindings   (e) only, with the app.exe already in the target dir; -BindingsRepo <dir>
#                    compares against that git work tree's app/ui/src/bindings instead
param(
    [string]$TargetDir = '',
    [int]$Jobs = 0,
    [ValidateSet('all', 'static', 'bindings')][string]$Only = 'all',
    [string]$BindingsRepo = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
if ($TargetDir) { $env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath((Join-Path $repo $TargetDir)) }
else { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
if ($Jobs -gt 0) { $env:CARGO_BUILD_JOBS = "$Jobs" }
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
$appDir = Join-Path $repo 'app'
$tauriDir = Join-Path $appDir 'src-tauri'
$work = Join-Path $repo ('target\gates\m9\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $work | Out-Null
$runStatic = $Only -eq 'all' -or $Only -eq 'static'
$runFull = $Only -eq 'all'
$runBindings = $Only -eq 'all' -or $Only -eq 'bindings'

$failures = @()
function Check($name, [scriptblock]$body) {
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}
function Note([string]$text) { Write-Host "      $text" }
function Latin1([string]$path) { [Text.Encoding]::GetEncoding(28591).GetString([IO.File]::ReadAllBytes($path)) }
# git's stdout only (its warnings would become errors under 'Stop'); the exit code in $gitCode.
function GitOut([string]$argline) {
    $o = @(cmd /c "git $argline 2>nul")
    $script:gitCode = $LASTEXITCODE
    $o
}

# Rust source with comments removed and string and char literals emptied (or kept, for build.rs,
# whose command names are strings), so a word in a comment or a string is not taken for code and
# braces and parentheses balance. Match() finds the bracket that closes the one at `open`.
if (-not ('M9RustText' -as [type])) {
    Add-Type -TypeDefinition @'
public static class M9RustText {
    static bool Ident(char c) { return char.IsLetterOrDigit(c) || c == '_'; }
    public static string Strip(string t, bool keepStrings) {
        var sb = new System.Text.StringBuilder(t.Length);
        int i = 0, n = t.Length;
        while (i < n) {
            char c = t[i];
            char nx = i + 1 < n ? t[i + 1] : '\0';
            if (c == '/' && nx == '/') { while (i < n && t[i] != '\n') i++; continue; }
            if (c == '/' && nx == '*') {
                int depth = 1; i += 2;
                while (i < n && depth > 0) {
                    if (t[i] == '/' && i + 1 < n && t[i + 1] == '*') { depth++; i += 2; }
                    else if (t[i] == '*' && i + 1 < n && t[i + 1] == '/') { depth--; i += 2; }
                    else i++;
                }
                sb.Append(' '); continue;
            }
            bool prefixOk = i == 0 || !Ident(t[i - 1]) || (t[i - 1] == 'b' && (i < 2 || !Ident(t[i - 2])));
            if (c == 'r' && (nx == '"' || nx == '#') && prefixOk) {
                int j = i + 1, hashes = 0;
                while (j < n && t[j] == '#') { hashes++; j++; }
                if (j < n && t[j] == '"') {
                    string end = "\"" + new string('#', hashes);
                    int close = t.IndexOf(end, j + 1, System.StringComparison.Ordinal);
                    int stop = close < 0 ? n : close + end.Length;
                    sb.Append(keepStrings ? t.Substring(i, stop - i) : "\"\"");
                    i = stop; continue;
                }
            }
            if (c == '"') {
                int j = i + 1;
                while (j < n && t[j] != '"') { if (t[j] == '\\') j++; j++; }
                int stop = System.Math.Min(n, j + 1);
                sb.Append(keepStrings ? t.Substring(i, stop - i) : "\"\"");
                i = stop; continue;
            }
            if (c == '\'') {
                int stop = -1;
                if (nx == '\\') { int close = i + 3 < n ? t.IndexOf('\'', i + 3) : -1; stop = close < 0 ? n : close + 1; }
                else if (i + 2 < n && t[i + 2] == '\'') stop = i + 3;
                else if (i + 3 < n && char.IsHighSurrogate(nx) && t[i + 3] == '\'') stop = i + 4;
                if (stop > 0) { sb.Append("' '"); i = stop; continue; }
            }
            sb.Append(c); i++;
        }
        return sb.ToString();
    }
    public static int Match(string t, int open) {
        char o = t[open];
        char cl = o == '{' ? '}' : o == '(' ? ')' : ']';
        int depth = 0;
        for (int i = open; i < t.Length; i++) {
            if (t[i] == o) depth++;
            else if (t[i] == cl && --depth == 0) return i;
        }
        return -1;
    }
}
'@
}
function RustCode([string]$path, [switch]$KeepStrings) { [M9RustText]::Strip((Get-Content $path -Raw), [bool]$KeepStrings) }

$cmdPattern = '#\[tauri::command\b[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?(async\s+)?(?:unsafe\s+)?fn\s+(\w+)'
$rs = @(Get-ChildItem (Join-Path $tauriDir 'src') -Recurse -Filter *.rs)

# ---- static checks -------------------------------------------------------------------------
if ($runStatic) {

# (g) every command is async. The attribute count must equal the parsed count, so a command the
# pattern cannot read fails the check instead of being skipped. Comments are not code.
Check "(g) 0 non-async #[tauri::command]" {
    $attrs = 0; $parsed = @()
    foreach ($f in $rs) {
        $code = RustCode $f.FullName
        $attrs += ([regex]::Matches($code, '#\[tauri::command\b')).Count
        foreach ($m in [regex]::Matches($code, $cmdPattern)) {
            $parsed += [pscustomobject]@{ file = $f.Name; name = $m.Groups[2].Value; async = $m.Groups[1].Success }
        }
    }
    $sync = @($parsed | Where-Object { -not $_.async })
    $handler = [regex]::Match((RustCode (Join-Path $tauriDir 'src\main.rs')), 'generate_handler!\[([^\]]*)\]').Groups[1].Value
    $registered = @([regex]::Matches($handler, 'commands::(\w+)') | ForEach-Object { $_.Groups[1].Value })
    $acl = @([regex]::Matches((RustCode (Join-Path $tauriDir 'build.rs') -KeepStrings), '"(\w+)",') | ForEach-Object { $_.Groups[1].Value })
    Note "$attrs #[tauri::command] attributes, $($parsed.Count) parsed, $($parsed.Count - $sync.Count) async, $($sync.Count) not async"
    $sync | ForEach-Object { Note "NOT ASYNC: $($_.file) $($_.name)" }
    $names = @($parsed | ForEach-Object { $_.name } | Sort-Object)
    $sameSets = ((@($registered | Sort-Object) -join ',') -eq ($names -join ',')) -and ((@($acl | Sort-Object) -join ',') -eq ($names -join ','))
    Note "registered in generate_handler!: $($registered.Count); in the build.rs ACL: $($acl.Count); same set: $sameSets"
    $attrs -gt 0 -and $attrs -eq $parsed.Count -and $sync.Count -eq 0 -and $sameSets
}

# A panic that escapes a command drops its reply, and Tauri's IPC script then re-sends the call
# over postMessage, so the body runs twice (docs/decisions/ipc.md). Every command but the probe
# that measures this must run its body through the guard. Read on code with comments and strings
# removed, per function body (brace-matched): the body must call guard::blocking, and the code
# outside the guarded closure must not panic in the obvious ways (panic-family macros, unwrap,
# expect). Heuristic beyond that: indexing or overflow outside the closure is not detected.
Check "every command body runs through guard::blocking (except panic_probe_unguarded)" {
    $unguarded = @(); $risky = @(); $n = 0
    foreach ($f in $rs) {
        $code = RustCode $f.FullName
        foreach ($m in [regex]::Matches($code, $cmdPattern)) {
            $name = $m.Groups[2].Value
            $n++
            $open = $code.IndexOf('{', $m.Index + $m.Length)
            $close = if ($open -ge 0) { [M9RustText]::Match($code, $open) } else { -1 }
            if ($close -lt 0) { $unguarded += "$($f.Name) $name (body not found)"; continue }
            if ($name -eq 'panic_probe_unguarded') { continue }
            $body = $code.Substring($open, $close - $open + 1)
            $calls = @([regex]::Matches($body, '\bguard::blocking\s*\(') | Sort-Object Index -Descending)
            if ($calls.Count -eq 0) { $unguarded += "$($f.Name) $name"; continue }
            $outside = $body
            foreach ($c in $calls) {
                $po = $c.Index + $c.Length - 1
                $pc = [M9RustText]::Match($outside, $po)  # the spans after $po are already removed
                if ($pc -lt 0) { $unguarded += "$($f.Name) $name (guard call not closed)"; continue }
                $outside = $outside.Remove($po, $pc - $po + 1)
            }
            $bad = @([regex]::Matches($outside, '\b(?:panic|unreachable|todo|unimplemented|assert(?:_eq|_ne)?)!|\.(?:unwrap|expect)\s*\(') | ForEach-Object { $_.Value })
            if ($bad.Count) { $risky += "$($f.Name) ${name}: $($bad -join ', ')" }
        }
    }
    $unguarded | ForEach-Object { Note "NO GUARD: $_" }
    $risky | ForEach-Object { Note "CAN PANIC OUTSIDE THE GUARD: $_" }
    Note "$n command(s); $($unguarded.Count) without the guard besides the probe; $($risky.Count) with panicking code outside it"
    $n -gt 0 -and $unguarded.Count -eq 0 -and $risky.Count -eq 0
}

# (f), first half: no shell or fs permission and no shell or fs plugin, in every place Tauri reads
# capabilities from: capabilities/**/* (tauri-build's pattern), inline capabilities under
# app.security.capabilities in any tauri config file (tauri.conf.json and platform files such as
# tauri.windows.conf.json), the app's permissions/ folder, and capabilities added from Rust
# (generate_context! arguments, add_capability, a custom capabilities_path_pattern).
function CapPermissions($node) {
    # A capability file holds one capability, a list of them, or {capabilities: [...]}.
    $names = @($node.PSObject.Properties.Name)
    $caps = if ($node -is [array]) { @($node) } elseif ($names -contains 'capabilities' -and $names -notcontains 'permissions') { @($node.capabilities) } else { @($node) }
    foreach ($c in $caps) {
        foreach ($p in @($c.permissions)) { if ($p -is [string]) { $p } elseif ($null -ne $p) { "$($p.identifier)" } }
    }
}
Check "(f) capabilities: 0 'shell:' and 0 'fs:'; dialog is the only plugin" {
    $capFiles = @(Get-ChildItem (Join-Path $tauriDir 'capabilities') -Recurse -File)
    $confFiles = @(Get-ChildItem $tauriDir -File | Where-Object { $_.Name -match '^(?i:tauri(\.[\w-]+)?\.conf\.json5?|tauri(\.[\w-]+)?\.toml)$' })
    $permDir = Join-Path $tauriDir 'permissions'
    $permFiles = @(if (Test-Path $permDir) { Get-ChildItem $permDir -Recurse -File })
    $shell = 0; $fs = 0
    foreach ($f in $capFiles + $confFiles + $permFiles) {
        $t = Get-Content $f.FullName -Raw
        $shell += ([regex]::Matches($t, 'shell:')).Count
        $fs += ([regex]::Matches($t, '\bfs:')).Count
    }
    $unparsed = @(); $perms = @(); $inline = 0; $refs = 0
    foreach ($f in $capFiles) {
        if ($f.Extension -ne '.json') { $unparsed += $f.FullName.Substring($tauriDir.Length + 1); continue }
        $perms += @(CapPermissions (Get-Content $f.FullName -Raw | ConvertFrom-Json))
    }
    foreach ($f in $confFiles) {
        if ($f.Extension -ne '.json') { $unparsed += $f.Name; continue }
        $conf = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $sec = $conf.app.security
        if ($sec -and @($sec.PSObject.Properties.Name) -contains 'capabilities') {
            foreach ($e in @($sec.capabilities)) {
                if ($e -is [string]) { $refs++ } else { $inline++; $perms += @(CapPermissions $e) }
            }
        }
    }
    $perms = @($perms | ForEach-Object { "$_" })
    $pluginPerms = @($perms | Where-Object { $_ -match ':' } | ForEach-Object { $_.Split(':')[0] } | Sort-Object -Unique)
    $src = (@($rs | ForEach-Object { RustCode $_.FullName }) + @(RustCode (Join-Path $tauriDir 'build.rs'))) -join "`n"
    $inits = @([regex]::Matches($src, '\.plugin\(\s*([\w:]+)') | ForEach-Object { $_.Groups[1].Value })
    $runtimeCaps = @([regex]::Matches($src, 'add_capability|capabilities_path_pattern|runtime_authority_mut|__allow_command|generate_context!\s*\(\s*[^\s)]') | ForEach-Object { $_.Value })
    $cargoToml = Get-Content (Join-Path $tauriDir 'Cargo.toml') -Raw
    $rustPlugins = @([regex]::Matches($cargoToml, '(?m)^(tauri-plugin-[\w-]+)\s*=') | ForEach-Object { $_.Groups[1].Value })
    $pkg = Get-Content (Join-Path $appDir 'package.json') -Raw | ConvertFrom-Json
    $jsDeps = @($pkg.dependencies.PSObject.Properties.Name) + @($pkg.devDependencies.PSObject.Properties.Name)
    $jsPlugins = @($jsDeps | Where-Object { $_ -like '@tauri-apps/plugin-*' })
    Note "scanned: $($capFiles.Count) capability file(s) (recursive), $($confFiles.Count) config file(s) ($(@($confFiles | ForEach-Object { $_.Name }) -join ', ')), $($permFiles.Count) permission file(s)"
    Note "$($perms.Count) permissions granted; inline capabilities in config: $inline; references: $refs; unparsed capability sources: $($unparsed.Count)"
    Note "'shell:' $shell, 'fs:' $fs; plugin permission prefixes: $($pluginPerms -join ', '); capabilities added from Rust: $($runtimeCaps.Count)"
    $unparsed | ForEach-Object { Note "UNPARSED: $_" }
    $runtimeCaps | ForEach-Object { Note "RUST CAPABILITY API: $_" }
    Note "plugins initialised: $($inits -join ', '); Rust plugin crates: $($rustPlugins -join ', '); npm plugins: $($jsPlugins -join ', ')"
    $capFiles.Count -gt 0 -and $shell -eq 0 -and $fs -eq 0 -and $unparsed.Count -eq 0 -and $runtimeCaps.Count -eq 0 -and
        ($pluginPerms -join ',') -eq 'dialog' -and $inits.Count -eq 1 -and $inits[0] -eq 'tauri_plugin_dialog::init' -and
        ($rustPlugins -join ',') -eq 'tauri-plugin-dialog' -and ($jsPlugins -join ',') -eq '@tauri-apps/plugin-dialog'
}

Check "strict CSP: 'self' only, no unsafe-inline or unsafe-eval, no remote origin" {
    $conf = Get-Content (Join-Path $tauriDir 'tauri.conf.json') -Raw | ConvertFrom-Json
    $csp = $conf.app.security.csp
    $pairs = @($csp.PSObject.Properties | ForEach-Object { "$($_.Name) $($_.Value)" })
    $all = $pairs -join '; '
    $remote = @([regex]::Matches($all, 'https?://[^\s;]+') | ForEach-Object { $_.Value } | Where-Object { $_ -ne 'http://ipc.localhost' })
    Note "csp: $all"
    Note "freezePrototype: $($conf.app.security.freezePrototype); withGlobalTauri: $($conf.app.withGlobalTauri)"
    $csp.'default-src' -eq "'self'" -and $csp.'script-src' -eq "'self'" -and $all -notmatch 'unsafe-' -and
        $remote.Count -eq 0 -and $all -notmatch '\*' -and $conf.app.security.freezePrototype -eq $true -and
        $conf.app.withGlobalTauri -eq $false
}

Check "WebView2-specific code only in src/webview2.rs" {
    $pattern = 'webview2_com|ICoreWebView2|with_webview|webview_version|\.controller\(\)|SetVirtualHostNameToFolderMapping'
    $hits = @($rs | Where-Object { $_.Name -ne 'webview2.rs' } | Select-String -Pattern $pattern)
    $hits | ForEach-Object { Note "OUTSIDE: $($_.Filename):$($_.LineNumber) $($_.Line.Trim())" }
    $hits.Count -eq 0
}

}

if ($runFull) {

# (a)
Check "(a) tauri >= 2.11.6" {
    $line = @(cmd /c "cargo tree -p app -i tauri --depth 0 2>&1") | Where-Object { $_ -match '^tauri v' } | Select-Object -First 1
    Note "cargo tree -p app -i tauri --depth 0: $line"
    if (-not ($line -match '^tauri v(\d+\.\d+\.\d+)')) { throw "no tauri version in cargo tree output" }
    [version]$Matches[1] -ge [version]'2.11.6'
}

Check "app crate: unit tests" {
    $out = @(cmd /c "cargo test -q -p app 2>&1")
    $out | Where-Object { $_ -match '^test result|FAILED|panicked' } | ForEach-Object { Note $_ }
    $LASTEXITCODE -eq 0
}
Check "app crate: clippy -D warnings (--no-deps: the app's own code)" {
    cmd /c "cargo clippy -q -p app --no-deps --all-targets -- -D warnings 2>&1" | ForEach-Object { Note $_ }
    $LASTEXITCODE -eq 0
}
Check "app crate: rustfmt --check" { cmd /c "cargo fmt -p app --check 2>&1" | ForEach-Object { Note $_ }; $LASTEXITCODE -eq 0 }
Check "UI: tsc typecheck" {
    Push-Location $appDir
    try { cmd /c "npm run -s typecheck 2>&1" | ForEach-Object { Note $_ }; $LASTEXITCODE -eq 0 } finally { Pop-Location }
}
# The same known answers bench.rs is held to (cargo test above); the self-test runs them again on
# the bundled code inside the webview.
Check "UI: checksum known answers (npm test)" {
    Push-Location $appDir
    try {
        $out = @(cmd /c "npm test -s 2>&1")
        $out | Where-Object { $_ -match 'known answers|FAIL|Error' } | ForEach-Object { Note $_ }
        $LASTEXITCODE -eq 0
    } finally { Pop-Location }
}

}

# ---- (b) the release build ------------------------------------------------------------------
$exe = Join-Path $target 'release\app.exe'
$buildLog = Join-Path $work 'tauri-build.log'
$built = $false
if ($runFull) {

Check "(b) npx tauri build --no-bundle exits 0" {
    $t0 = Get-Date
    Push-Location $appDir
    try { cmd /c "npx --no-install tauri build --no-bundle > `"$buildLog`" 2>&1"; $code = $LASTEXITCODE } finally { Pop-Location }
    $secs = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
    Get-Content $buildLog | Select-Object -Last 4 | ForEach-Object { Note $_ }
    Note "exit $code in $secs s; log $buildLog"
    $fresh = (Test-Path $exe) -and (Get-Item $exe).LastWriteTime -ge $t0.AddSeconds(-1)
    if (-not $fresh) { Note "app.exe missing or not rebuilt: $exe" }
    $script:built = $code -eq 0 -and $fresh
    $script:built
}

# (f), the capabilities tauri-build resolved from the files in this build (gen/schemas).
Check "(f) the capabilities this build resolved: 0 'shell:', 0 'fs:', dialog the only plugin" {
    if (-not $built) { throw 'no fresh build from (b)' }
    $path = Join-Path $tauriDir 'gen\schemas\capabilities.json'
    $resolved = Get-Content $path -Raw | ConvertFrom-Json
    $ids = @($resolved.PSObject.Properties.Name)
    $perms = @($ids | ForEach-Object { CapPermissions $resolved.$_ } | ForEach-Object { "$_" })
    $prefixes = @($perms | Where-Object { $_ -match ':' } | ForEach-Object { $_.Split(':')[0] } | Sort-Object -Unique)
    $bad = @($perms | Where-Object { $_ -match '^(shell|fs):' })
    Note "gen/schemas/capabilities.json: capabilities $($ids -join ', '); $($perms.Count) permissions; plugin prefixes: $($prefixes -join ', '); shell/fs: $($bad.Count); file written $((Get-Item $path).LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))"
    $ids.Count -gt 0 -and $bad.Count -eq 0 -and ($prefixes -join ',') -eq 'dialog'
}

# (f), second half: the dist the build just embedded.
Check "(f) built dist: 0 googleapis, 0 http(s):// outside the listed exceptions" {
    $dist = Join-Path $appDir 'ui\dist'
    # Strings that are not origins: nothing loads from them.
    $allowed = [ordered]@{
        'https://react.dev/errors/'           = "React's error-decoder link, pasted into minified error messages"
        'http://www.w3.org/2000/svg'          = 'XML namespace name (react-dom createElementNS), never fetched'
        'http://www.w3.org/1999/xlink'        = 'XML namespace name (react-dom setAttributeNS), never fetched'
        'http://www.w3.org/1998/Math/MathML'  = 'XML namespace name (react-dom createElementNS), never fetched'
        'http://www.w3.org/XML/1998/namespace' = 'XML namespace name (react-dom setAttributeNS), never fetched'
    }
    $files = @(Get-ChildItem $dist -Recurse -File)
    $google = 0; $counts = @{}; $bad = @(); $licence = 0
    foreach ($f in $files) {
        $t = Latin1 $f.FullName
        $google += ([regex]::Matches($t, 'googleapis|gstatic')).Count
        $licence += ([regex]::Matches($t, '@license|@preserve|/\*!')).Count
        foreach ($m in [regex]::Matches($t, 'https?://[A-Za-z0-9._~:/?#@!$&''()*+,;=%-]*')) {
            $u = $m.Value
            $hit = @($allowed.Keys | Where-Object { $u -eq $_ -or ($_.EndsWith('/') -and $u.StartsWith($_)) }) | Select-Object -First 1
            if ($hit) { $counts[$hit] = 1 + [int]$counts[$hit] } else { $bad += "$($f.Name): $u" }
        }
    }
    Note "$($files.Count) files in dist; googleapis/gstatic: $google; licence comments: $licence"
    foreach ($k in $allowed.Keys) { Note ("exception x{0}: {1} ({2})" -f [int]$counts[$k], $k, $allowed[$k]) }
    $bad | ForEach-Object { Note "REMOTE: $_" }
    Note "http(s):// outside the exceptions: $($bad.Count)"
    $index = Get-Content (Join-Path $dist 'index.html') -Raw
    $external = [regex]::Matches($index, '(src|href)="(?!/assets/)[^"]*"').Count
    Note "index.html references outside /assets/: $external"
    $fonts = @($files | Where-Object { $_.Extension -eq '.woff2' }).Count
    Note "bundled woff2 fonts: $fonts"
    $files.Count -gt 0 -and $google -eq 0 -and $bad.Count -eq 0 -and $external -eq 0 -and $fonts -eq 7
}

# ---- (c), (d), (h): the self-test ------------------------------------------------------------
# Every check the UI's self-test must report, by name, so none can be dropped quietly.
$requiredChecks = @(
    'webgl2_context', 'webgl_max_texture_size', 'ipc_transfers_verified', 'ipc_numbers_present',
    'ipc_single_16MB_within_limit', 'panic_probe_returns_error', 'ping_after_panic',
    'alive_after_unguarded_panic', 'channel_complete_in_order', 'channel_batch_period_50ms',
    'channel_gap_near_period', 'channel_batched', 'checksum_known_answers', 'bridge_exact_floats',
    'bridge_op_roundtrip_exact', 'bridge_undo', 'step_bar_five_steps', 'no_acoustic_numbers',
    'fonts_bundled_and_loaded', 'csp_blocks_inline_script', 'csp_blocks_eval', 'prototype_frozen'
)
$st = $null
$stJson = Join-Path $work 'selftest.json'
Check "(c) app.exe --selftest exits 0 and writes its report" {
    if (-not $built) { throw 'no fresh app.exe from (b)' }
    $t0 = Get-Date
    $p = Start-Process $exe -ArgumentList '--selftest', "`"$stJson`"" -PassThru `
        -RedirectStandardOutput (Join-Path $work 'selftest.stdout.txt') -RedirectStandardError (Join-Path $work 'selftest.stderr.txt')
    $null = $p.Handle  # Windows PowerShell 5.1 reports no ExitCode unless the handle was taken early
    if (-not $p.WaitForExit(240000)) { $p.Kill(); throw 'the self-test did not exit within 240 s' }
    $p.WaitForExit()
    $secs = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
    if (-not (Test-Path $stJson)) { throw "no report at $stJson (exit $($p.ExitCode))" }
    $script:st = Get-Content $stJson -Raw | ConvertFrom-Json
    $checks = @($st.checks.PSObject.Properties)
    $passed = @($checks | Where-Object { $_.Value -eq $true }).Count
    $names = @($checks | ForEach-Object { $_.Name })
    $missing = @($requiredChecks | Where-Object { $names -notcontains $_ })
    Note "exit $($p.ExitCode) after $secs s; UI checks $passed/$($checks.Count) passed; failed: $(@($st.failed) -join ', ')"
    Note "required checks missing from the report: $($missing.Count) $($missing -join ', ')"
    if ($st.error) { Note "error: $($st.error.code) $($st.error.message)" }
    Note "machine $($st.meta.machine), tauri $($st.meta.tauri_version), $($st.meta.webview.engine) $($st.meta.webview.version), written $($st.meta.written_utc)"
    $p.ExitCode -eq 0 -and $st.ok -eq $true -and $passed -eq $checks.Count -and $missing.Count -eq 0
}

Check "(c) webgl.renderer contains 'NVIDIA'; MAX_TEXTURE_SIZE read" {
    if (-not $st) { throw 'no self-test report' }
    Note "renderer: $($st.webgl.renderer)"
    Note "MAX_TEXTURE_SIZE $($st.webgl.max_texture_size), MAX_3D_TEXTURE_SIZE $($st.webgl.max_3d_texture_size), unmasked $($st.webgl.unmasked)"
    "$($st.webgl.renderer)" -match 'NVIDIA' -and [int]$st.webgl.max_texture_size -gt 0
}

Check "(c) IPC numbers present, transfers verified, recorded in docs/decisions/ipc.md" {
    if (-not $st) { throw 'no self-test report' }
    $ipc = $st.ipc
    Note ("this run: single_16MB_ms {0} ({1} MB/s), all_bands_98MB_ms {2} ({3} MB/s), concurrent {4} ms, ping median {5} ms" -f `
        $ipc.single_16MB_ms, $ipc.single_MBps, $ipc.all_bands_98MB_ms, $ipc.all_bands_MBps, $ipc.all_bands_98MB_concurrent_ms, $ipc.ping_median_ms)
    Note "bytes: single $($ipc.single_bytes), all bands $($ipc.all_bands_bytes) in $($ipc.all_bands_files) files; checksums verified: $($ipc.verified)"
    $doc = Get-Content (Join-Path $repo 'docs\decisions\ipc.md') -Raw
    $rec = @{}
    foreach ($key in 'single_16MB_ms', 'all_bands_98MB_ms', 'machine', 'date') {
        $m = [regex]::Match($doc, "(?m)^\|\s*``$key``\s*\|\s*([^|]+?)\s*\|")
        if ($m.Success) { $rec[$key] = $m.Groups[1].Value }
    }
    Note "recorded: single_16MB_ms $($rec.single_16MB_ms), all_bands_98MB_ms $($rec.all_bands_98MB_ms), machine $($rec.machine), date $($rec.date)"
    $num = { param($x) $d = 0.0; [double]::TryParse("$x", [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$d) -and $d -gt 0 }
    $present = (& $num $ipc.single_16MB_ms) -and (& $num $ipc.all_bands_98MB_ms) -and $ipc.single_bytes -eq 16470804 -and
        $ipc.all_bands_bytes -eq 98439868 -and $ipc.verified -eq $true
    $recorded = (& $num $rec.single_16MB_ms) -and (& $num $rec.all_bands_98MB_ms) -and $rec.machine -eq $env:COMPUTERNAME -and
        $rec.date -match '^\d{4}-\d{2}-\d{2}$'
    $present -and $recorded
}

Check "(c) single_16MB_ms <= 500, or the virtual-host fallback is built and measured faster" {
    if (-not $st) { throw 'no self-test report' }
    $single = [double]$st.ipc.single_16MB_ms
    if ($single -le 500) { Note "$single ms <= 500 ms: raw ipc::Response stays the path, no fallback needed"; return $true }
    $vh = $st.ipc.virtual_host
    Note "$single ms > 500 ms: the virtual-host fallback is required; measured: $(if ($vh) { $vh.single_16MB_ms } else { 'none' })"
    $vh -and [double]$vh.single_16MB_ms -lt $single
}

# The run-event channel, measured in the webview: the period the backend uses, the gaps between
# batch arrivals, and the batch count against the run's length. A 10 ms or a 100 ms batcher fails.
Check "run events: batched at 50 ms (measured), complete and in order" {
    if (-not $st) { throw 'no self-test report' }
    $ch = $st.channel
    $period = [double]$ch.backend.batch_period_ms
    $expected = [double]$ch.backend.elapsed_ms / 50
    $gap = [double]$ch.median_gap_ms
    Note ("{0} lines over {1:N1} ms: {2} batches (expected {3:N1}, allowed {4:N1} to {5:N1}); median gap {6} ms (allowed 40 to 75); backend period {7} ms" -f `
        $ch.lines, [double]$ch.backend.elapsed_ms, $ch.batches, $expected, (0.7 * $expected), (1.3 * $expected + 2), $gap, $period)
    Note "received $($ch.received) in order: $($ch.in_order); batches contiguous: $($ch.batches_contiguous); last marked: $($ch.saw_last); send failures: $($ch.backend.stats.send_failures)"
    $period -eq 50 -and $gap -ge 40 -and $gap -le 75 -and $ch.batches -ge 0.7 * $expected -and $ch.batches -le 1.3 * $expected + 2 -and
        $ch.received -eq $ch.lines -and $ch.in_order -eq $true -and $ch.batches_contiguous -eq $true -and $ch.saw_last -eq $true -and
        [int]$ch.backend.stats.send_failures -eq 0
}

Check "checksum known answers, run inside the webview on the bundled code" {
    if (-not $st) { throw 'no self-test report' }
    $kat = @($st.checksum_known_answers)
    $ok = @($kat | Where-Object { $_.ok -eq $true }).Count
    Note "$ok of $($kat.Count) cases match checksum-kat.json"
    $kat.Count -ge 6 -and $ok -eq $kat.Count
}

Check "(d) panic probe -> error to the UI, then ping ok" {
    if (-not $st) { throw 'no self-test report' }
    $pp = $st.panic_probe
    $pu = $st.panic_probe_unguarded
    Note "guarded: resolved $($pp.resolved), error $($pp.error.code): $($pp.error.message); ping after: $($pp.ping_after)"
    Note "unguarded (recorded, not gated): $($pu.outcome); its body ran $($pu.body_runs) time(s); ping after: $($pu.ping_after); a 16.5 MB transfer afterwards: $($pu.single_16MB_after_ms) ms"
    $pp.resolved -eq $false -and $pp.error.code -eq 'PANIC' -and $pp.ping_after -eq 'pong'
}

Check "(h) the step bar shows the five steps (the app's own DOM self-test)" {
    if (-not $st) { throw 'no self-test report' }
    $want = @('Geometry', 'Materials', 'Sources & receivers', 'Simulate', 'Results')
    $steps = @($st.dom.steps)
    $visible = @($st.dom.steps_visible)
    Note "steps: $($steps -join ' | '); visible: $($visible -join ', '); current: $($st.dom.current_step)"
    Note "menus: $(@($st.dom.menu) -join ', '); dock tabs: $(@($st.dom.dock_tabs) -join ', ')"
    Note "numbers with an acoustic unit in the visible text: $($st.dom.acoustic_number_matches)"
    ($steps -join '|') -eq ($want -join '|') -and @($visible | Where-Object { $_ -ne $true }).Count -eq 0 -and
        $st.dom.acoustic_number_matches -eq 0
}

}

# ---- (e) the bindings --------------------------------------------------------------------------
# Regenerated into a scratch folder (a failing gate never rewrites the committed files), then
# compared as git blobs: `git hash-object --path=<bindings path>` applies that path's attributes
# and line-ending conversion, which is what `git diff` compares. So a checkout with CRLF endings
# (core.autocrlf=true) still matches the LF files the generator writes, and a real change does
# not. When the files are tracked, the regenerated blob must equal the index's and HEAD's too.
if ($runBindings) {

if ($Only -eq 'bindings') { $built = Test-Path $exe; Note "using the existing $exe" }
Check "(e) bindings regenerate to the same git blobs; git diff clean" {
    if (-not $built) { throw "no app.exe to dump the schemas with: $exe" }
    $root = if ($BindingsRepo) { (Resolve-Path $BindingsRepo).Path } else { $repo }
    $out = Join-Path $work 'bindings'
    Push-Location $appDir
    try { cmd /c "npm run -s bindings -- --app `"$exe`" --out `"$out`" 2>&1" | ForEach-Object { Note $_ }; $code = $LASTEXITCODE } finally { Pop-Location }
    if ($code -ne 0) { throw "npm run bindings exited $code" }
    Note "compared against the git work tree $root"
    $same = $true; $tracked = 0
    foreach ($name in 'schema.json', 'ipc.json', 'schema.ts', 'ipc.ts') {
        $rel = "app/ui/src/bindings/$name"
        $file = Join-Path $root $rel
        if (-not (Test-Path $file)) { Note "$name missing from $root"; $same = $false; continue }
        $regenFile = Join-Path $out $name
        $regen = @(GitOut "-C `"$root`" hash-object `"--path=$rel`" -- `"$regenFile`"")[0]
        $wt = @(GitOut "-C `"$root`" hash-object -- `"$rel`"")[0]
        $idx = @(GitOut "-C `"$root`" rev-parse -q --verify `":$rel`"")[0]
        $head = @(GitOut "-C `"$root`" rev-parse -q --verify `"HEAD:$rel`"")[0]
        # (Kept out of a "$(...)" string: Windows PowerShell 5.1 mangles the nested quotes there.)
        $attr = @(GitOut "-C `"$root`" check-attr eol -- `"$rel`"")[0]
        $eol = "$attr" -replace '^.*: eol: ', ''
        $crlf = (Latin1 $file).Contains("`r`n")
        if ($idx) { $tracked++ }
        $ok = $regen -and $regen -eq $wt -and (-not $idx -or $regen -eq $idx) -and (-not $head -or $regen -eq $head)
        if (-not $ok) { $same = $false }
        Note ("{0,-12} blob {1} {2}; eol attribute {3}; working file has CRLF: {4}; in index: {5}; in HEAD: {6}" -f $name,
            "$wt".Substring(0, [Math]::Min(12, "$wt".Length)),
            $(if ($ok) { 'identical' } else { "DIFFERS (regenerated $regen, index $idx, HEAD $head)" }),
            $eol, $crlf, [bool]$idx, [bool]$head)
    }
    $null = GitOut "-C `"$root`" diff --quiet -- app/ui/src/bindings"; $diff = $gitCode
    Note "git: $tracked tracked file(s) under app/ui/src/bindings; git diff exit $diff"
    $same -and $diff -eq 0
}

}

Write-Host "`nwork folder: $work"
if ($Only -ne 'all') {
    if ($failures.Count) { Write-Host "M9 PARTIAL RUN (-Only $Only) FAILED: $($failures.Count) check(s)"; exit 1 }
    Write-Host "M9 PARTIAL RUN (-Only $Only): its checks passed; this is not a gate pass"; exit 0
}
if ($failures.Count) { Write-Host "M9 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "M9 PASSED"; exit 0
