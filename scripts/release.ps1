<#
.SYNOPSIS
  PGB1 一键发版：构建 → 签名 → 回填 latest.json → 创建 GitHub Release → 校验更新链路。

.DESCRIPTION
  把 docs/打包发布指南.md 第 9 节的手工流程自动化，消除三个已知翻车点：
    1. signature 手工粘贴（漏贴/贴错 → updater 静默验签失败）
    2. pub_date 手填
    3. latest.json 的 url 与 GitHub 实际资产名不一致（→ updater 404）

.PARAMETER SkipBuild
  跳过编译，直接用 target 目录里已有的产物走发布流程（重跑发布步骤时用）。

.PARAMETER DryRun
  只做构建与 latest.json 回填，不创建 Release。

.EXAMPLE
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "你的密钥密码"
  .\scripts\release.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# ── 常量（SSOT：文件名格式与打包指南一致） ──────────────────────────────
$RepoSlug      = 'ArareF/PGB1'
$KeyPath       = Join-Path $env:USERPROFILE '.tauri\PGB1.key'
$BundleDir     = 'src-tauri\target\release\bundle\nsis'
$LatestJson    = 'latest.json'

# PS 5.1 坑：$ErrorActionPreference = 'Stop' 时，只要重定向原生命令的 stderr
# （2>$null / 2>&1），PowerShell 就把 stderr 包成 NativeCommandError 抛出 ——
# 重定向本身触发异常，而不是命令真的失败了。凡是需要吞掉 stderr 或读退出码的
# 原生调用，都走这个包装：临时把 EAP 降回 Continue，退出码自己判。
function Invoke-Native {
    param([Parameter(Mandatory)][scriptblock]$Command)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & $Command } finally { $ErrorActionPreference = $prev }
}

function Step($msg) { Write-Host "`n=== $msg ===" -ForegroundColor Cyan }
function Ok($msg)   { Write-Host "  [OK] $msg" -ForegroundColor Green }
function Die($msg)  { Write-Host "  [失败] $msg" -ForegroundColor Red; exit 1 }

# ── 0. 前置检查 ────────────────────────────────────────────────────────
Step '前置检查'
if (-not (Test-Path 'package.json')) { Die '请在仓库根目录执行本脚本' }
foreach ($cmd in @('node', 'npm', 'gh', 'git')) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { Die "找不到命令：$cmd" }
}
if (-not (Test-Path $KeyPath)) { Die "签名私钥不存在：$KeyPath（丢失则无法签发新版本）" }
Ok '工具链与私钥就位'

$dirty = git status --porcelain
if ($dirty) { Write-Host "  [注意] 工作区有未提交改动，Release 将基于当前 HEAD 提交打标签" -ForegroundColor Yellow }

# ── 1. 版本号一致性（七处） ────────────────────────────────────────────
Step '版本号一致性校验'
npm run check:version-sync
if ($LASTEXITCODE -ne 0) { Die '版本号不一致，先修齐再发版' }

$Version = (Get-Content 'package.json' -Raw -Encoding UTF8 | ConvertFrom-Json).version
$Tag     = "v$Version"
Ok "目标版本：$Tag"

# 退出码 0 = Release 已存在；非 0（含 "release not found"）= 可以发
$releaseExitCode = Invoke-Native { gh release view $Tag --repo $RepoSlug 2>&1 | Out-Null; $LASTEXITCODE }
if ($releaseExitCode -eq 0) {
    Die "Release $Tag 已存在。要重发请先 gh release delete $Tag --repo $RepoSlug --cleanup-tag"
}
Ok "Release $Tag 尚未创建，可以发布"

# ── 2. 构建 ────────────────────────────────────────────────────────────
$SetupExe = Join-Path $BundleDir "PGB1_${Version}_x64-setup.exe"
$SigFile  = "$SetupExe.sig"

if ($SkipBuild) {
    Step '跳过构建（-SkipBuild）'
} else {
    Step '构建（vue-tsc + check:all + vite build + cargo build + NSIS 打包）'
    if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
        # 不把密码写进仓库：从环境变量取，没有就当场问
        $sec = Read-Host -AsSecureString '请输入私钥密码'
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD =
            [Runtime.InteropServices.Marshal]::PtrToStringAuto(
                [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec))
    }
    # 指南踩坑：Get-Content 不加 .Trim() 会带尾换行导致密钥解析失败
    $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $KeyPath -Raw).Trim()

    npm run tauri build
    if ($LASTEXITCODE -ne 0) { Die '构建失败' }
}

if (-not (Test-Path $SetupExe)) { Die "找不到安装包：$SetupExe" }
if (-not (Test-Path $SigFile))  { Die "找不到签名文件：$SigFile（说明签名被静默跳过，检查密钥环境变量）" }
Ok "安装包：$SetupExe ($([math]::Round((Get-Item $SetupExe).Length / 1MB, 1)) MB)"
Ok "签名文件：$SigFile"

# ── 3. 回填 latest.json ────────────────────────────────────────────────
Step '回填 latest.json'
$Signature = ((Get-Content $SigFile -Raw) -replace '\s', '')
if (-not $Signature) { Die '签名文件为空' }
# -replace 的替换串里 $ 有特殊含义（$1/$&），base64 理论上不含 $，仍做防御性转义
$SignatureEsc = $Signature -replace '\$', '$$$$'
$PubDate = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
$Url     = "https://github.com/$RepoSlug/releases/download/$Tag/PGB1_${Version}_x64-setup.exe"

# PS 5.1 的 Get-Content 默认按系统 ANSI 代码页解码，读 UTF-8 中文会变乱码，
# 再原样写回就是双重损坏（notes 直接进用户的更新弹窗）。必须显式指定 UTF8。
$json = Get-Content $LatestJson -Raw -Encoding UTF8
$json = $json -replace '"version":\s*"[^"]*"',   "`"version`": `"$Version`""
$json = $json -replace '"pub_date":\s*"[^"]*"',  "`"pub_date`": `"$PubDate`""
$json = $json -replace '"url":\s*"[^"]*"',       "`"url`": `"$Url`""
$json = $json -replace '"signature":\s*"[^"]*"', "`"signature`": `"$SignatureEsc`""

# UTF-8 无 BOM：notes 是中文，带 BOM 会让部分解析器出错
[System.IO.File]::WriteAllText(
    (Join-Path (Get-Location) $LatestJson), $json,
    (New-Object System.Text.UTF8Encoding($false)))

$check = Get-Content $LatestJson -Raw -Encoding UTF8 | ConvertFrom-Json
if ($check.version -ne $Version)                    { Die 'latest.json 版本回填失败' }
if ($check.platforms.'windows-x86_64'.signature -match 'REPLACE_ME') { Die '签名仍是占位符' }
# 乱码探针：UTF-8 中文被按 ANSI 误读后必然出现 Ã / Â / â€ / ï¼ 这类残片。
# notes 会显示在用户的更新弹窗里，宁可在这里拦下也不要发出去。
if ($check.notes -match '[ÃÂ]|â€|ï¼') {
    Die 'latest.json 的 notes 出现乱码特征，编码读取有问题，已中止发布'
}
Ok "version=$Version  pub_date=$PubDate"
Ok "signature 已写入（$($Signature.Length) 字符）"

# notes 是长中文串，走 --notes-file 而不是命令行参数，避开引号与编码问题
$NotesFile = Join-Path $env:TEMP "pgb1-release-notes-$Version.md"
[System.IO.File]::WriteAllText($NotesFile, $check.notes, (New-Object System.Text.UTF8Encoding($false)))

if ($DryRun) {
    Step 'DryRun：到此为止，未创建 Release'
    Write-Host "  产物：$SetupExe"
    Write-Host "  latest.json 已就绪，可手动 gh release create"
    exit 0
}

# ── 4. 提交 latest.json（仓库惯例：build: update latest.json for vX） ──
Step '提交 latest.json'
Invoke-Native {
    git add $LatestJson
    git commit -m "build: update latest.json for $Tag" 2>&1 | Out-Host
}
$Branch = (git rev-parse --abbrev-ref HEAD).Trim()
$Target = (git rev-parse HEAD).Trim()

# gh release create --target 要求目标提交在远端已存在，本地 commit 不推上去
# 会导致打标签失败 —— 而这一步在构建之后，失败等于白等一轮编译。
Step "推送 $Branch"
Invoke-Native { git push origin $Branch 2>&1 | Out-Host }
if ($LASTEXITCODE -ne 0) {
    Invoke-Native { Start-Sleep -Seconds 3; git push origin $Branch 2>&1 | Out-Host }
    if ($LASTEXITCODE -ne 0) { Die "推送失败。Release 的目标提交必须已存在于远端，请手动 git push origin $Branch 后加 -SkipBuild 重跑" }
}
# 回读远端，确认目标提交真的到了 GitHub
$remoteSha = Invoke-Native { (git ls-remote origin $Branch 2>&1) -split '\s+' | Select-Object -First 1 }
if ($remoteSha -ne $Target) { Die "远端 $Branch 指向 $remoteSha，与本地 HEAD $Target 不一致，Release 会打错标签" }
Ok "Release 目标提交：$Target（远端已确认）"

# ── 5. 创建 Release ────────────────────────────────────────────────────
Step "创建 Release $Tag"
gh release create $Tag $SetupExe $LatestJson `
    --repo $RepoSlug `
    --target $Target `
    --latest `
    --title "$Tag" `
    --notes-file $NotesFile
if ($LASTEXITCODE -ne 0) { Die 'Release 创建失败' }
Ok 'Release 已创建'

# ── 6. 校验更新链路 ────────────────────────────────────────────────────
Step '校验更新链路'
$assetsRaw = Invoke-Native { gh release view $Tag --repo $RepoSlug --json assets 2>&1 }
if (-not $assetsRaw) { Die '读取 Release 资产清单失败' }
$assets = ($assetsRaw | ConvertFrom-Json).assets
$names  = $assets | ForEach-Object { $_.name }
Write-Host "  资产清单：$($names -join ', ')"

$expectedAsset = "PGB1_${Version}_x64-setup.exe"
if ($names -notcontains $expectedAsset) { Die "资产名与 latest.json 的 url 不一致，updater 会 404。实际：$($names -join ', ')" }
if ($names -notcontains 'latest.json')  { Die 'latest.json 未上传，updater 拿不到版本信息' }
Ok "资产名与 url 逐字符一致：$expectedAsset"

# updater 实际请求的就是这个地址（tauri.conf.json 的 endpoints）
$endpoint = "https://github.com/$RepoSlug/releases/latest/download/latest.json"
try {
    $live = Invoke-RestMethod -Uri $endpoint -MaximumRedirection 5
    if ($live.version -eq $Version) { Ok "端点已生效：$endpoint 返回 $($live.version)" }
    else { Write-Host "  [注意] 端点当前返回 $($live.version)，GitHub CDN 可能有缓存延迟，稍后重试" -ForegroundColor Yellow }
} catch {
    Write-Host "  [注意] 端点暂不可达（CDN 延迟），稍后手动访问确认：$endpoint" -ForegroundColor Yellow
}

Write-Host "`n发版完成。" -ForegroundColor Green
Write-Host "在已安装旧版的机器上：设置 → 关于 → 检查更新，应提示升级到 $Version。"
Write-Host "（或重启程序，启动 3 秒后会自动弹更新提示）"
