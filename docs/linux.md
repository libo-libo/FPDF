# fpdf

Linux x64 PDF 阅读器，使用 Rust 编写。支持连续阅读、多标签、全文搜索、目录编辑、高亮与下划线、夜间与护眼模式，以及基于 pi agent 的 AI 辅助阅读。PDF 解析使用独立 PDFium，无需安装 WPS。

[下载最新版](https://github.com/libo-libo/FPDF/releases/latest) · [更新记录](https://github.com/libo-libo/FPDF/blob/main/CHANGELOG.md)

![夜间模式与目录页码](https://raw.githubusercontent.com/libo-libo/FPDF/main/docs/fpdf-outline-pages-preview.png)

## 从 GitHub 下载和安装

1. 打开 [FPDF 仓库](https://github.com/libo-libo/FPDF)，点击右侧 **Releases**；也可以直接打开[版本下载页](https://github.com/libo-libo/FPDF/releases/latest)。
2. 进入最新版本（当前 **v0.8.5**），展开下方 **Assets**。点击文件名下载，无需 Git 或 GitHub 账号。
3. 选择下面的安装包或免安装包。`Source code (zip)` / `Source code (tar.gz)` 是开发用源码，不能直接作为阅读器运行。

| 文件 | 使用方法 |
| --- | --- |
| `fpdf-0.8.5-linux-x64.run` | 执行 `chmod +x` 后直接运行，按普通用户权限安装到 `~/.local/opt/fpdf`。 |
| `fpdf-0.8.5-linux-x64.tar.gz` | 解压后进入版本目录，执行 `./install.sh`。不安装也可直接运行 `./fpdf`。 |
| `fpdf-0.8.5-linux-x64.sha256` | 下载校验值，使用以下命令核对。 |

要求 x86_64 Linux、glibc 2.28 或更新版本，已验证 AlmaLinux 8 / EL8。需要 X11 图形会话或 SSH X11 转发，以及 GTK 3.22 或更新版本。发行包自带 PDFium、Node.js、AI bridge、中文字体及 X11 运行库，无需安装 Rust、Node.js 或开发工具。X11 键盘支持优先使用系统自带的 `libxkbcommon` / `libxkbcommon-x11`，以便与系统核心库版本匹配；`lib/` 中的副本只在系统缺少相应库时使用。安装和运行均使用普通用户权限，不需要 root；除可选的 AI 服务外也不需要联网。**保留整个程序目录**，包括 `fpdf-bin`、`libpdfium.so`、`libfpdf-glx-compat.so`、`lib/`、`ai/` 和许可证。`fpdf` 是启动脚本，它会加载同目录下的 GLX 兼容库后再运行 `fpdf-bin`；只复制其中某一个文件无法运行。

在当前目录计算文件 SHA-256，与同版本的 `.sha256` 文件对照：

```sh
sha256sum -c fpdf-0.8.5-linux-x64.sha256
```

自解压安装包可直接运行；`--prefix` 和 `--bin-dir` 会传给 `install.sh`：

```sh
chmod +x fpdf-0.8.5-linux-x64.run
./fpdf-0.8.5-linux-x64.run
./fpdf-0.8.5-linux-x64.run --prefix "$HOME/apps/fpdf"
```

免安装包解压及安装：

```sh
tar -xzf fpdf-0.8.5-linux-x64.tar.gz
cd fpdf-0.8.5-linux-x64
./install.sh
```

默认安装到 `~/.local/opt/fpdf`，并创建启动器 `~/.local/bin/fpdf`、桌面入口、图标和配置目录 `~/.config/fpdf/`。升级会保留配置。若 `~/.local/bin` 不在 `PATH` 中：

```sh
export PATH="$HOME/.local/bin:$PATH"
```

不安装也可以从解压目录直接运行：

```sh
./fpdf /path/to/document.pdf
```

通过 SSH 使用 X11 转发时：

```sh
ssh -X user@server
fpdf /path/to/document.pdf
```

若桌面环境输出 `xdg-document-portal` 或 DBus 警告，可用 `dbus-run-session -- fpdf /path/to/document.pdf` 启动；文件选择器使用系统 GTK 3，不依赖 portal。选定文件或点击“取消”、按 `Esc` 后，选择窗口会立即关闭并返回阅读器。16 bit X11 转发画面可能出现渐变色带，这是显示链路色深造成的，不会修改 PDF 内容；需要精确核对颜色时请将客户端 X server 设为 24/32 bit。

## 基本阅读

顶部标签栏可在同一窗口打开多个 PDF。点击 `＋` 或按 `Ctrl+T` 新建标签；“打开文件”支持一次选择多个 PDF，拖入文件也会添加标签。点击标签切换，点击 `×`、鼠标中键或按 `Ctrl+W` 关闭；`Ctrl+Tab` / `Ctrl+Shift+Tab` 切换前后标签。各标签保留页码、缩放、搜索和 AI 对话历史。同一文件在本窗口重复打开会定位到已有标签。

“文件 → 新窗口”或 `Ctrl+N` 可打开独立窗口；“在新窗口打开 PDF…”直接选择另一个文档。各窗口分别保留阅读位置、查找结果、选区和 AI 会话。

左侧导航窗格集中提供“顶部工具栏”开关；关闭后可在顶部导航条重新打开。顶部工具栏下边缘可用鼠标上下拖动调整高度，左侧导航窗格边缘可左右拖动调整宽度。顶部导航条中的“界面字号”可统一调整各个界面的文字大小，范围为 75%–175%，用于适配不同分辨率；该设置不改变 PDF 页面缩放，关闭左侧导航窗格后仍可使用。

- 点击“打开文件”、按 `Ctrl+O`，或把 PDF 拖入窗口。加密文档按提示输入密码。
- 默认连续阅读，可切换“单页阅读”。右侧实心滚动条支持拖动定位。
- 点击缩略图、目录名称或右侧页码跳转；底部可输入页码。“后退 / 前进”返回先前位置。
- 顶部及底部均有缩小 / 放大；百分比支持输入 **10%–500%** 后按 Enter，也可拖动滑块或使用 `Ctrl+滚轮`。
- “实际大小”为 100%；“适合宽度”随窗口调整，“适合页面”完整显示当前页；旋转每次 90°。
- 连续模式缩小后按可用宽度自动显示每行 **2、4、8 页**；放大恢复单列。点击小页面选为当前页。
- PDF 正文目录中带内部链接的章节可直接点击跳转，支持直接目标和命名目标。点击“后退”或按 `Alt+←` 返回原页和滚动位置，`Alt+→` 前进。
- `Ctrl+F` 或“全文搜索”在整个 PDF 中查找，右侧显示页码、上下文片段和进度；点击结果定位，支持区分大小写与全词匹配。“文本面板”仍可复制当前页文字。

## 高亮与下划线标注

点击“下划线”可给选中文字添加下划线，也可先开启工具再连续拖选；`Ctrl+Shift+U` 切换，`Esc` 退出。双击单词或拖动选择同页多行文字，再点击“高亮标注”；也可先开启工具再连续拖选。`Ctrl+F2` 切换高亮工具，`Esc` 退出。

PDF 区域按 `Ctrl+Z` 撤销本次打开文档后添加的标注，按 `Ctrl+Y` 或 `Ctrl+Shift+Z` 重做；“文件”菜单也提供撤销/重做。输入框获得焦点时，`Ctrl+A/C/X/V/Z/Y` 等快捷键作用于文字，文档导航不会抢走这些按键。

“标注颜色”保留黄、绿、蓝、粉常用色；选择“自定义颜色…”可点选调色盘、拖动亮度条或输入 RGB（0–255）。高亮和下划线分别保留颜色。标注保存为标准 PDF 标注，重新打开或使用其他兼容阅读器时仍可显示。夜间及深色自定义主题保留标注色相。加密、含数字签名或被外部修改的 PDF 会停止保存，并提示原因。

## 文档目录：折叠、改名和页码

左侧“文档目录”显示 PDF 自带书签。章节名称与目标页码之间用 `*` 引导，所有层级页码统一右对齐。点击名称或页码跳转；无跳转目标的分组不显示页码。

点击三角形折叠或展开，也可“展开全部 / 折叠全部”。**双击目录名称即可原位改名**：Enter 或点击其他位置保存到原 PDF，Esc 取消。“编辑目录”支持新增章节/子章节、目标页码、层级和顺序、删除及撤销。可保存到原 PDF 或另存副本。加密或含数字签名的 PDF 暂不支持目录编辑。

## 夜间、护眼与自定义 RGB

顶部背景菜单提供默认、夜间、护眼、羊皮纸四种主题。底部也有背景与护眼入口。“自定义 RGB…”直接显示调色盘：点击选色、拖动亮度条，或输入 R/G/B（各 0–255）。选色立即预览，“确定”保留，“取消”恢复原色。

主题只改变显示，不修改 PDF；重启后保留在 `~/.config/fpdf/reading-theme.txt`。

## AI 辅助阅读

无需先打开 PDF，即可点击“连接”测试 API、获取模型列表，或使用“保存并连接”。AI 使用 pi agent core / pi-ai，默认模型 **GLM-5.3-Flash**。**发布文件不包含作者的 AI 接口地址、密钥或账号**，需要自行配置兼容 OpenAI 的服务。

复制 `agent.example.txt` 为 `~/.config/fpdf/agent.txt`，填写两行：

```text
https://your-api.example/v1
YOUR_API_KEY
```

新版推荐直接在 AI 面板点击 **API 配置**，填写接口地址与密钥，选择“保存并连接”。也可用环境变量 `FPDF_AGENT_CONFIG` 指定配置文件，或把文件放在程序旁。不要把真实配置提交到 GitHub。

点击“AI 辅助阅读”即可看到问题输入框。选中文字后可点击工具栏或右键中的“针对选区提问”，问题会包含选区、附近上下文和页码；也可直接问整份文档。输入后点击“发送问题”（未连接时为“连接并提问”），或按 `Ctrl+Enter`。“总结当前页”“概览文档”快速获取要点，流式回答中的“第 N 页”可跳转引用。

### 选词翻译：单次与长开

在 PDF 页面双击单词，或按住左键拖动选择同页多行文字，蓝色高亮实时更新。两个按钮保留单次操作，并分别提供“长期开启”复选框：

| 选项 | 行为 |
| --- | --- |
| 结合上下文翻译 | 根据附近文字翻译选区，保留专有名词、数字和公式。 |
| 结合上下文翻译并详解 | 先翻译，再解释术语、指代、难句结构与含义。 |
| 长期开启 | 勾选后，每次选词或拖选松手即自动翻译。两种长开模式互斥。 |

无需额外提问或发送。处理时显示“翻译进行中…”，译文实时更新。新选区取消旧任务；“停止本次”只停止当前翻译，不关闭长开。单次按钮不改变长开状态。长开在本次运行中保持，重启后需重新开启。

![夜间模式 AI 辅助阅读](https://raw.githubusercontent.com/libo-libo/FPDF/main/docs/fpdf-ai-preview.png)

普通阅读在本机完成。AI 问答或翻译会把问题、文件名、目录、选区及相关页面文本发送给自己配置的服务。Agent 只提供 PDF 文本读取工具，每次最多读 5 页，没有命令执行或文件写入工具。扫描图片文字目前需要另行 OCR。

## 快捷键和常见问题

| 快捷键 | 功能 |
| --- | --- |
| `Ctrl+O` / `Ctrl+F` | 在标签页打开 PDF / 全 PDF 查找 |
| `Ctrl+T` / `Ctrl+W` | 新建 / 关闭标签页 |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | 下一个 / 上一个标签页 |
| `Ctrl+N` | 新窗口 |
| `Ctrl+Shift+U` | 切换下划线工具；已有选区时立即标注 |
| `Ctrl+F2` | 切换高亮工具；已有选区时立即标注 |
| `PageUp` / `PageDown` | 上一页 / 下一页 |
| `Ctrl+Home` / `Ctrl+End` | 首页 / 末页 |
| `Alt+Left` / `Alt+Right` | 后退 / 前进 |
| `Ctrl+0` / `Ctrl+1` | 适合宽度 / 实际大小 |
| `Ctrl++` / `Ctrl+-` / `Ctrl+滚轮` | 缩放 |
| `Ctrl+Enter` | 在文档问答输入框发送 |

- `GLXBadContextTag` 或无法打开窗口：发行版自带的 `libfpdf-glx-compat.so` 会绕过旧版 Mesa 的交换间隔报错，请从安装目录用 `./fpdf` 或 `~/.local/bin/fpdf` 启动，并确认该文件与 `fpdf-bin` 仍在；若手工运行 `fpdf-bin`，需要自行设置 `LD_PRELOAD`。同时检查 `DISPLAY`、X11 转发和客户端 X server；GL/EGL 不可用时可尝试 `LIBGL_ALWAYS_SOFTWARE=1 fpdf`。
- 缺少 `libpdfium.so`：完整安装或解压，保留整个目录。
- AI 无法连接：保留 `ai/`，检查配置格式、服务地址、密钥与模型权限。
- 文件选择窗口不出现、关闭后仍停留在屏幕上或无法取消：确认系统安装 GTK 3.22 或更新版本，并使用当前发行包；也可直接将 PDF 路径作为参数传入。
- 中文显示为方框：确认安装目录中的 `fonts/wqy-microhei.ttc` 未被删除。
- 目录保存失败：检查写权限、外部修改、密码与数字签名，或另存副本。

暂不提供 OCR、跨页拖选、附注文本编辑、页面内容编辑或打印。单文件最多 512 MiB。全文查找最多显示每页 2,000 条、全篇 20,000 条，达到上限会提示。

## 从源码构建

安装 Rust stable（支持 edition 2024）、Node.js 24 与 npm、Python 3、C/C++ 构建工具，以及 GTK 3、X11 和 OpenGL 开发包。在 Linux 终端执行：

```sh
git clone https://github.com/libo-libo/FPDF.git
cd FPDF
npm ci --prefix ai-runtime
node scripts/build-ai.mjs
./scripts/fetch-pdfium.sh
./scripts/fetch-linux-runtime.sh
./scripts/fetch-node.sh
./scripts/build-linux.sh
```

输出为 `dist/fpdf-0.8.5-linux-x64/`、同名 `.tar.gz` 和 `.run`。PDFium、Node.js 和 X11 运行库地址与 SHA-256 固定在脚本中，依赖由 `Cargo.lock` / `package-lock.json` 锁定。首次构建需要联网下载依赖与许可证。`scripts/build-linux.sh` 会执行格式检查、Clippy、Rust 测试、AI bridge 测试、EL8 ABI 检查和许可证收集。

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
npm test --prefix ai-runtime
cargo run -- tests/fixtures/reading-demo.pdf
```

图形验证需要可用的 X11 显示，可运行 `cargo run -- --smoke-ui tests/fixtures/reading-demo.pdf --screenshot window.png --features` 并检查截图。

## 源码结构与许可证

`src/app.rs` 为界面，`src/reading.rs` 为布局与主题，`src/pdf.rs` 封装 PDFium，`src/worker.rs` 在专用线程执行原生 PDF 操作，`src/ai_reader.rs` 与 `ai-runtime/bridge.mjs` 实现 AI 通信。测试和自建样例在 `tests/`，构建脚本在 `scripts/`，Linux 安装脚本在 `packaging/`。

应用代码采用 [MIT](https://github.com/libo-libo/FPDF/blob/main/LICENSE) 许可证。PDFium、Node.js、pi、X11 运行库及 Rust 依赖许可证随包提供，详见 `THIRD_PARTY_NOTICES.txt` 和 `licenses/`。界面与素材独立实现，发行包不包含 WPS 程序或其私有 DLL。反馈请使用 [GitHub Issues](https://github.com/libo-libo/FPDF/issues)，贡献约定见 [AGENTS.md](https://github.com/libo-libo/FPDF/blob/main/AGENTS.md)。
