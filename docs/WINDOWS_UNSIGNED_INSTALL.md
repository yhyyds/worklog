# Windows 未签名安装包与公司电脑部署

## 现状

Worklog 尚未配置受信任的 Authenticode 代码签名证书。`tauri.conf.json` 中的 `publisher` 只属于安装包元数据，不会让 Windows 显示可信发行者。

因此每次重新构建产生的新文件哈希都可能被 Microsoft Defender SmartScreen 视为未知应用。过去某个未签名版本能够安装，不代表后续新哈希会自动继承信誉。

## 本次提供的格式

| 文件 | 用途 | WebView2 |
| --- | --- | --- |
| `*-no-webview2-setup.exe` | 当前用户 NSIS，小体积 | 目标机必须已有 |

从 1.3.0 起，日常发布不再生成内置 WebView2 版本或 MSI；需要时再单独恢复相应构建。

## 完整性材料

- `SHA256SUMS.txt`：安装包的 SHA-256。
- `BUILD-INFO.txt`：版本、提交、构建时间、文件大小、SHA-256、Authenticode 状态和签名者。

用户可把安装包、SHA-256 和构建信息提交给 IT，由 IT 按文件哈希、路径或内部软件发布系统批准。不要关闭 Defender，也不要使用自动解除文件阻止或修改公司策略的脚本。

## 真正消除未知发行者

需要以下任一方案：

1. 公司内部 CA 签发并已部署信任的代码签名证书。
2. 公共 OV/EV 代码签名证书。
3. Azure Trusted Signing 等受信任云签名服务。

获得证书后，应同时签署应用程序和安装包，使用 SHA-256 时间戳，并在 CI 中用 `signtool verify /pa` 强制验签。
