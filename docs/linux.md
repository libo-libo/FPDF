# fpdf Linux x64

fpdf 是一个使用 Rust 编写的 PDF 阅读器。本发行包面向 x86_64 Linux，提供连续阅读、多标签、全文搜索、目录编辑、高亮与下划线，以及可选的 AI 辅助阅读。

## 系统要求

- x86_64 Linux，glibc 2.28 或更新版本；已验证 AlmaLinux 8 / EL8。
- X11 图形会话或 SSH X11 转发，并设置有效的 `DISPLAY`。
- 系统提供 `libX11.so.6`、`libGL.so.1`、`libxkbcommon.so.0`、`libxcb.so.1` 和 GTK 3（`libgtk-3.so.0`，用于文件选择窗口）。
- 安装和运行均使用普通用户权限，不需要 root，也不需要联网。
- AI 功能需要能够访问自行配置的 OpenAI 兼容接口；不连接 AI 时可完全离线阅读。

发行包已经包含 PDFium、Node.js、AI bridge、中文字体，以及 X11 启动所需的 `libxkbcommon-x11.so.0` 和 `libxcb-xkb.so.1`。可执行文件会从自身目录加载这两个库；不要只复制 `fpdf` 可执行文件，必须保留整个安装目录。

本版本已在 AlmaLinux 8.10 用户环境、Mesa 23.1.4 和 Xvnc 的 24-bit/16-bit X11 显示下通过窗口启动与渲染冒烟测试。SSH 转发的实际画面仍取决于客户端 X server。

## 安装

解压后进入版本目录：

```sh
tar -xzf fpdf-0.8.5-linux-x64.tar.gz
cd fpdf-0.8.5-linux-x64
./install.sh
```

默认安装到 `~/.local/opt/fpdf`，并创建：

- 启动器：`~/.local/bin/fpdf`
- 桌面入口：`~/.local/share/applications/fpdf.desktop`
- 图标：`~/.local/share/icons/hicolor/256x256/apps/fpdf.png`
- 配置：`~/.config/fpdf/`

如果 `~/.local/bin` 不在 `PATH` 中：

```sh
export PATH="$HOME/.local/bin:$PATH"
```

自解压版本可直接安装，并会把额外参数传给 `install.sh`：

```sh
chmod +x fpdf-0.8.5-linux-x64.run
./fpdf-0.8.5-linux-x64.run
./fpdf-0.8.5-linux-x64.run --prefix "$HOME/apps/fpdf"
```

`install.sh` 也接受 `--prefix DIR` 和 `--bin-dir DIR`。升级会保留 `~/.config/fpdf/`。

## 直接运行

不安装也可以从解压目录运行：

```sh
./fpdf
./fpdf /path/to/document.pdf
```

通过 SSH 使用 X11 转发：

```sh
ssh -X user@server
fpdf /path/to/document.pdf
```

如果桌面环境输出 `xdg-document-portal` 或 DBus 警告，可用下面的方式启动；文件选择器使用系统 GTK 3，不依赖 portal：

```sh
dbus-run-session -- fpdf /path/to/document.pdf
```

当前服务器使用 16 bit X11 转发画面时，渐变、阴影和半透明区域可能出现色带。这是显示链路色深造成的，不会修改 PDF 内容。需要精确核对颜色时，请将客户端 X server 设为 24/32 bit。

## AI 配置

阅读功能不需要网络。AI 使用 OpenAI 兼容接口，配置文件为 `~/.config/fpdf/agent.txt`，内容为两行：

```text
https://your-api.example/v1
YOUR_API_KEY
```

也可在应用内“API 配置”中填写并保存。`FPDF_AGENT_CONFIG` 可指定其他配置文件。发行包不包含任何 API 地址、密钥或账号。

## 验证与故障排查

```sh
fpdf --help
fpdf --render document.pdf --output page.png --page 1 --width 1200
ldd ~/.local/opt/fpdf/fpdf
```

- `未找到 PDF 引擎`：确认 `libpdfium.so` 与 `fpdf` 位于同一目录。
- `未找到 AI 所需的 Node.js`：确认 `ai/node` 和 `ai/bridge.mjs` 存在且有执行权限。
- 无法打开窗口：检查 `DISPLAY`、X11 转发和客户端 X server；确认安装目录中的两个 X11 运行时库仍在。GL/EGL 不可用时可尝试 `LIBGL_ALWAYS_SOFTWARE=1 fpdf`。
- 文件选择窗口不出现：确认系统安装 GTK 3.22 或更新版本；也可直接将 PDF 路径作为参数传入。
- 中文显示为方框：确认安装目录中的 `fonts/wqy-microhei.ttc` 未被删除。

## 卸载

```sh
~/.local/opt/fpdf/uninstall.sh
```

添加 `--purge` 会同时删除 `~/.config/fpdf/` 中保存的 AI 配置、主题和日志。用户 PDF 不会被删除。

## 许可证

应用代码采用 MIT 许可证。PDFium、Node.js、pi、Rust 依赖、X11 运行时和中文字体的许可证随包提供，详见 `THIRD_PARTY_NOTICES.txt`、`licenses/` 和 `ai/licenses/`。
