# Builds Shelf for Android on this PC.
#
# `tauri android build` compiles the Rust library fine, then tries to symlink it into the Android
# project, which Windows refuses outside Developer Mode. So this lets Tauri compile, copies the
# library in itself, and runs Gradle with Tauri's Rust step skipped.
#
#   .\build-apk.ps1            debug APK for phones (arm64), for sideloading: dist\Shelf.apk
#   .\build-apk.ps1 -Emulator  the same plus x86_64, for the Android emulator
#   .\build-apk.ps1 -Release   signed App Bundle for Google Play: dist\Shelf.aab
param([switch]$Emulator, [switch]$Release)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$env:JAVA_HOME = 'C:\Program Files\Microsoft\jdk-17.0.20.101-hotspot'
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\28.2.13676358"

if ($Release -and -not (Test-Path "$env:USERPROFILE\.shelf-signing\keystore.properties")) {
    throw 'No upload key in ~\.shelf-signing. See README, "Releasing".'
}

$all = @{
    aarch64 = @{ triple = 'aarch64-linux-android'; abi = 'arm64-v8a'; arch = 'arm64' }
    armv7   = @{ triple = 'armv7-linux-androideabi'; abi = 'armeabi-v7a'; arch = 'arm' }
    x86_64  = @{ triple = 'x86_64-linux-android'; abi = 'x86_64'; arch = 'x86_64' }
}
$names = if ($Release) { 'aarch64', 'armv7', 'x86_64' } elseif ($Emulator) { 'aarch64', 'x86_64' } else { , 'aarch64' }

$jni = 'src-tauri\gen\android\app\src\main\jniLibs'
Remove-Item $jni -Recurse -Force -ErrorAction SilentlyContinue
foreach ($n in $names) {
    $t = $all[$n]
    # The Rust library is always built with the dev profile: a release-profile build compiles
    # helper programs that Smart App Control blocks on this PC (see README).
    $so = "src-tauri\target\$($t.triple)\debug\libshelf_lib.so"
    Write-Host "Compiling Rust for $n..."
    # Expected to fail at the symlink step; success is judged by the .so below.
    $ErrorActionPreference = 'Continue'
    npx tauri android build --apk --debug --target $n 2>&1 | Select-String 'error\[|error:|Finished' | ForEach-Object { Write-Host "  $_" }
    $ErrorActionPreference = 'Stop'
    if (-not (Test-Path $so)) { throw "Rust build for $n produced no library." }
    New-Item -ItemType Directory -Force "$jni\$($t.abi)" | Out-Null
    Copy-Item $so "$jni\$($t.abi)\libshelf_lib.so"
}

$abis = ($names | ForEach-Object { $all[$_].abi }) -join ','
$archs = ($names | ForEach-Object { $all[$_].arch }) -join ','
$rusts = $names -join ','
$task, $skip, $out = if ($Release) {
    'bundleUniversalRelease', 'rustBuildUniversalRelease', 'src-tauri\gen\android\app\build\outputs\bundle\universalRelease\*.aab'
} else {
    'assembleUniversalDebug', 'rustBuildUniversalDebug', 'src-tauri\gen\android\app\build\outputs\apk\universal\debug\*.apk'
}
Push-Location src-tauri\gen\android
try {
    .\gradlew.bat $task -x $skip "-PabiList=$abis" "-ParchList=$archs" "-PtargetList=$rusts" --console=plain -q
    if ($LASTEXITCODE -ne 0) { throw 'Gradle build failed.' }
} finally { Pop-Location }

$built = Get-ChildItem $out | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$dest = if ($Release) { 'dist\Shelf.aab' } else { 'dist\Shelf.apk' }
New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item $built.FullName $dest -Force
Write-Host ("Built {0} ({1:N1} MB)" -f $dest, ((Get-Item $dest).Length / 1MB))
