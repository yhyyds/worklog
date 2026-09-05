# Worklog 无损迁移到本机 Codex

## 原则

先复制、再校验、最后接管。验证完成前不删除旧工作目录、远程分支、数据库、Vault 或安装包。

## 1. 完整 Git 备份

```powershell
$backupRoot = "D:\Worklog-Migration-2026-09-05"
New-Item -ItemType Directory -Force $backupRoot | Out-Null

git clone --mirror https://github.com/yhyyds/worklog.git "$backupRoot\worklog-mirror.git"
git -C "$backupRoot\worklog-mirror.git" fsck --full
git -C "$backupRoot\worklog-mirror.git" bundle create "$backupRoot\worklog-complete.bundle" --all
git -C "$backupRoot\worklog-mirror.git" bundle verify "$backupRoot\worklog-complete.bundle"
```

把 Bundle 再复制到第二块磁盘或可信云存储。源码 ZIP 不能替代 Git 镜像。

## 2. 创建本机工作副本

```powershell
New-Item -ItemType Directory -Force C:\Dev | Out-Null
Set-Location C:\Dev
git clone https://github.com/yhyyds/worklog.git
Set-Location .\worklog
git fetch --all --tags --prune
git switch codex/0.9.1-local-handoff
git merge-base --is-ancestor 7d2fdfc203ff55358abfd7b9dfdebb5140a27fb3 HEAD
git status --short
```

`merge-base --is-ancestor` 返回退出码 0 才表示基线关系正确。不要切换到 `main` 开始本轮开发。

## 3. 备份运行数据

先从系统托盘彻底退出 Worklog，并在设置中记录实际数据库目录、Vault 和日记根目录。

```powershell
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$backup = "D:\Worklog-Data-Backup\$stamp"
New-Item -ItemType Directory -Force $backup | Out-Null

robocopy "D:\实际Worklog数据目录" "$backup\data" /E /COPY:DAT /DCOPY:DAT /R:2 /W:1 /XJ
if ($LASTEXITCODE -ge 8) { throw "数据库备份失败" }

$appState = Join-Path $env:APPDATA "cn.worklog.desktop"
robocopy $appState "$backup\app-state" /E /COPY:DAT /DCOPY:DAT /R:2 /W:1 /XJ
if ($LASTEXITCODE -ge 8) { throw "应用状态备份失败" }

robocopy "D:\实际ObsidianVault" "$backup\vault" /E /COPY:DAT /DCOPY:DAT /R:2 /W:1 /XJ
if ($LASTEXITCODE -ge 8) { throw "Vault 备份失败" }
```

不要使用 `/MIR`。确认 Vault 副本包含 `.obsidian` 和 `.worklog-backups`。

生成校验清单：

```powershell
$manifest = Join-Path (Split-Path $backup -Parent) "$stamp-manifest.csv"
Get-ChildItem -LiteralPath $backup -Recurse -Force -File | ForEach-Object {
  [PSCustomObject]@{
    Path = $_.FullName.Substring($backup.Length).TrimStart("\")
    Size = $_.Length
    SHA256 = (Get-FileHash $_.FullName -Algorithm SHA256).Hash
  }
} | Export-Csv $manifest -NoTypeInformation -Encoding UTF8
```

## 4. 锁定依赖

0.9.1 未提交锁文件。首次安装依赖后立即生成并提交：

```powershell
npm install
cargo generate-lockfile --manifest-path src-tauri/Cargo.toml
npm run check
cargo test --locked --manifest-path src-tauri/Cargo.toml
git add package-lock.json src-tauri/Cargo.lock
git commit -m "chore: lock 0.9.1 dependencies for local handoff"
git push
```

之后前端统一使用 `npm ci`。

## 5. 打开 Codex

在 Codex 桌面端打开 `C:\Dev\worklog`，或者：

```powershell
Set-Location C:\Dev\worklog
codex
```

首次会话使用 `docs/LOCAL_CODEX_PROMPT.md` 中的提示词。

## 6. 验收后才能继续

- 三处版本号为 0.9.1。
- 前端与 Rust 测试通过。
- 开发模式能读取备份副本中的相同任务、时间线和设置。
- 同一个 Vault 中人工文字、受管理区块和隐藏目录完整。
- 能生成当前 0.9.1 安装包。
- 本地 Codex 正确复述数据安全规则与下一轮两项需求。
- 完成一次本地 PR 后仍保留原始备份。