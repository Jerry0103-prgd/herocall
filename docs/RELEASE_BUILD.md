# 发布构建说明

## macOS（Apple Silicon）

在 macOS 构建机的仓库根目录执行（项目已固定使用 Node 版 Tauri CLI）：

```bash
cd frontend
npm run tauri -- build
```

如果构建机另行安装了 `cargo-tauri` 子命令，也可在 `src-tauri/` 运行 `cargo tauri build`。两种方式均使用同一份 `tauri.conf.json`。

构建产物位于 `src-tauri/target/release/bundle/macos/`（`.app`）和 `src-tauri/target/release/bundle/dmg/`（`.dmg`）。产物在分发前应进行代码签名与公证；未签名构建仅用于本地验收。

应用的 SQLite 数据库由 Tauri `app_data_dir()` 写入 macOS 系统应用数据目录，不使用仓库或开发目录。首次启动应创建数据库、运行全部迁移并显示可跳过的初始化向导。

## Windows

Windows 安装包由 `.github/workflows/build-windows.yml` 在 `windows-latest` 上构建。它会执行 `npm ci --prefix frontend` 与 `npm run tauri -- build`，并上传 MSI 和 NSIS `.exe` 产物。

当前工作流生成未签名测试安装包。正式公开发布前，应在受控环境配置 Windows 代码签名证书，并将签名私钥仅作为 CI Secret 提供，绝不提交到仓库。
