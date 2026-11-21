# 发布与自签名

`.github/workflows/release.yml` 在推送 `v*` 标签时构建并发布 macOS、Windows、Linux 三个平台的产物，并使用**同一套本地自签名密钥**签名：

- macOS：用 PKCS#12 导入临时钥匙串，给 `.app` 做 codesign。
- Windows：用同一个 PKCS#12 给 exe 和 NSIS 安装包做 Authenticode 签名。
- Linux：从 PEM 私钥生成 tar.gz 的 detached SHA-256 签名。
- 发布页附带公钥证书，供用户验证/信任。

自签名证书不会让 Gatekeeper/SmartScreen 自动放行；它只是让产物有可验证的签名。正式分发仍建议 Developer ID / 代码签名证书并做公证。

## 本地签名文件

本机签名身份默认位于：

```text
~/Library/Application Support/PeninsulaSigning/
├── cert.pem
├── key.pem
├── identity.p12
└── password
```

这些文件不进入仓库。丢失后无法恢复同一签名身份，只能重新生成，用户需要重新信任。

## GitHub Secrets

在仓库 **Settings → Secrets and variables → Actions → New repository secret** 添加：

| Secret | 内容 |
| --- | --- |
| `SIGNING_P12_BASE64` | `identity.p12` 的 base64 |
| `SIGNING_P12_PASSWORD` | `password` 文件内容（不要带换行） |
| `SIGNING_KEY_PEM_BASE64` | `key.pem` 的 base64（Linux 签名用） |
| `SIGNING_CERT_PEM_BASE64` | `cert.pem` 的 base64（发布页公钥 + Linux 验签） |
| `SIGNING_IDENTITY` | 可选；默认 `Peninsula Local Signing` |

### 用网页添加

macOS 上逐个复制：

```sh
cd "$HOME/Library/Application Support/PeninsulaSigning"

base64 -i identity.p12 | pbcopy          # SIGNING_P12_BASE64
pbcopy < password                        # SIGNING_P12_PASSWORD
base64 -i key.pem | pbcopy               # SIGNING_KEY_PEM_BASE64
base64 -i cert.pem | pbcopy              # SIGNING_CERT_PEM_BASE64
```

然后到 GitHub 仓库的 Secrets 页面粘贴对应值。`SIGNING_IDENTITY` 只有改了证书 CN 才需要设置。

### 用 gh CLI 添加

```sh
cd "$HOME/Library/Application Support/PeninsulaSigning"

gh secret set SIGNING_P12_BASE64 < <(base64 -i identity.p12)
gh secret set SIGNING_P12_PASSWORD < password
gh secret set SIGNING_KEY_PEM_BASE64 < <(base64 -i key.pem)
gh secret set SIGNING_CERT_PEM_BASE64 < <(base64 -i cert.pem)
gh secret set SIGNING_IDENTITY --body "Peninsula Local Signing"
```

Linux 上生成 base64 时用 `base64 -w0`（`-i` 是 macOS 参数）：

```sh
base64 -w0 identity.p12 > identity.p12.b64
```

## 触发发布

```sh
git tag v0.1.3
git push origin v0.1.3
```

`Release` workflow 会：

1. 三个平台并行构建；
2. 用同一个自签名身份签名；
3. 上传 artifacts；
4. 在 tag 上创建 GitHub Release，附上所有产物和 `SHA256SUMS.txt`。

只构建、不发布时可以用 **Actions → Release → Run workflow**；此时只上传 artifacts，不创建 Release。

## 产物

- `Peninsula-<version>-macos-<arch>.zip`
- `Peninsula-<version>-windows-x64-setup.exe`
- `Peninsula-<version>-windows-x64-portable.zip`
- `Peninsula-<version>-linux-x86_64.tar.gz`
- `Peninsula-<version>-linux-x86_64.tar.gz.sig`
- `Peninsula-Local-Signing.pem` / `.cer` / `.pub.pem`（公钥证书/公钥）
- `SHA256SUMS.txt`

Linux 验签：

```sh
openssl dgst -sha256 -verify Peninsula-Local-Signing.pub.pem \
  -signature Peninsula-<version>-linux-x86_64.tar.gz.sig \
  Peninsula-<version>-linux-x86_64.tar.gz
```

macOS 用户首次打开自签名 `.app` 可能需要在 Finder 中右键“打开”，或在终端执行：

```sh
xattr -dr com.apple.quarantine Peninsula.app
```

也可以把 `.cer` 加入钥匙串并设为信任。Windows 用户需要把 `.cer` 安装到“受信任的根证书颁发机构”和“受信任的发布者”，否则会显示未知发布者/SmartScreen 提示。Linux 运行需要与构建相同的 X11/Wayland/OpenGL 库。

## 安全说明

- 私钥放在 GitHub Actions Secrets 中，不要提交到仓库。
- Workflow 不会打印私钥、p12 或密码；GitHub 也会对 secrets 做日志掩码。
- 任何有仓库 Secrets 管理权限的人都能用这个身份签名；请限制管理员。
- 如果私钥泄露或被删除，重新生成一套身份会让既有用户的信任失效；请备份 `PeninsulaSigning` 目录。
- 自签名仍然不能消除 macOS Gatekeeper / Windows SmartScreen 的警告；要彻底消除需要正式代码签名证书与公证。
