"""Generate the public Chinese user guide. Requires reportlab and a Chinese TTF."""
from pathlib import Path
from html import escape
import os
import re
from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle, Image, PageBreak

ROOT = Path(__file__).resolve().parents[1]
VERSION = re.search(r'^version = "([^"]+)"', (ROOT / 'Cargo.toml').read_text('utf-8'), re.M)[1]
OUTPUT = ROOT / 'output/pdf' / f'fpdf-{VERSION}-user-guide-zh-CN.pdf'
OUTPUT.parent.mkdir(parents=True, exist_ok=True)
font_path = Path(os.environ.get('FPDF_GUIDE_FONT', 'C:/Windows/Fonts/simhei.ttf'))
if not font_path.is_file():
    raise SystemExit('Set FPDF_GUIDE_FONT to a Chinese TrueType font file.')
pdfmetrics.registerFont(TTFont('Guide', str(font_path)))
pdfmetrics.registerFontFamily('Guide', normal='Guide', bold='Guide', italic='Guide', boldItalic='Guide')
RED = colors.HexColor('#DA3336')
INK = colors.HexColor('#27313D')
MUTED = colors.HexColor('#697586')
PALE = colors.HexColor('#F5F6F8')
WIDTH = A4[0] - 100
styles = {
    'body': ParagraphStyle('body', fontName='Guide', fontSize=10.2, leading=16, textColor=INK, spaceAfter=8, wordWrap='CJK'),
    'h1': ParagraphStyle('h1', fontName='Guide', fontSize=23, leading=31, textColor=INK, spaceAfter=15, wordWrap='CJK'),
    'h2': ParagraphStyle('h2', fontName='Guide', fontSize=13, leading=20, textColor=RED, spaceBefore=9, spaceAfter=6, wordWrap='CJK'),
    'small': ParagraphStyle('small', fontName='Guide', fontSize=8.7, leading=13, textColor=MUTED, spaceAfter=6, wordWrap='CJK'),
    'cover': ParagraphStyle('cover', fontName='Guide', fontSize=39, leading=48, textColor=RED, spaceAfter=12),
    'subtitle': ParagraphStyle('subtitle', fontName='Guide', fontSize=18, leading=27, textColor=INK, spaceAfter=15),
    'caption': ParagraphStyle('caption', fontName='Guide', fontSize=8.5, leading=12, textColor=MUTED, alignment=TA_CENTER, spaceBefore=5, spaceAfter=10),
}
story = []
HEADINGS = ['开始使用', '安装与阅读', '文档目录与阅读外观', 'AI 配置与文档问答', '选区翻译：单次与长开', '快捷键与常见问题']

def p(text, style='body'):
    return Paragraph(text, styles[style])

def add(text, style='body'):
    story.append(p(text, style))

def section(number, title):
    if number > 1:
        story.append(PageBreak())
    add(f'{number:02d}  {title}', 'h1')

def table(rows, widths=None):
    cells = [[p(escape(str(cell)).replace('\n', '<br/>')) for cell in row] for row in rows]
    result = Table(cells, colWidths=widths or [120, WIDTH-120], hAlign='LEFT')
    result.setStyle(TableStyle([
        ('BACKGROUND', (0,0), (-1,0), PALE), ('VALIGN',(0,0),(-1,-1),'TOP'),
        ('LEFTPADDING',(0,0),(-1,-1),9), ('RIGHTPADDING',(0,0),(-1,-1),9),
        ('TOPPADDING',(0,0),(-1,-1),6), ('BOTTOMPADDING',(0,0),(-1,-1),3),
        ('LINEBELOW',(0,0),(-1,-1),0.4,colors.HexColor('#DEE2E7')),
    ]))
    story.extend([result, Spacer(1,9)])

def screenshot(name, width=WIDTH):
    image = Image(str(ROOT / 'docs' / name))
    image.drawHeight *= width/image.drawWidth
    image.drawWidth = width
    story.append(image)

section(1, '开始使用')
add('fpdf', 'cover')
add('PDF 阅读器 · 中文使用说明', 'subtitle')
add(f'版本 {VERSION}  |  Windows x64  |  2026-10-22', 'small')
add('连续阅读、目录编辑、选词翻译。界面采用简洁的工具栏与左右侧栏布局。普通阅读无需联网，AI 功能由使用者自行配置。')
screenshot('fpdf-underline-preview.png', 450)
add('夜间模式实际界面：下划线写入 PDF，颜色调色盘支持点选、亮度和 RGB 数值。', 'caption')
table([
    ['三步开始', '操作'],
    ['1. 安装或解压', '运行安装程序；或完整解压免安装包，保留整个目录。'],
    ['2. 打开 PDF', '打开 fpdf，点击“打开文件”，也可拖入 PDF 或按 Ctrl+O。'],
    ['3. 按需开启 AI', '阅读 PDF 不必配置 AI；需要总结或翻译时，再填写自己的 API 配置。'],
])
add('阅读指引：安装与阅读 02 / 文档目录与外观 03 / AI 配置 04 / 单次和长开翻译 05 / 快捷键与问题 06', 'small')

section(2, '安装与阅读')
add('从 GitHub 下载 Release 包', 'h2')
add('1. 打开 github.com/libo-libo/FPDF，点击右侧 Releases；也可直接访问 github.com/libo-libo/FPDF/releases/latest。', 'small')
add(f'2. 选择相应版本（本说明对应 v{VERSION}），展开页面下方 Assets，点击 setup.exe（安装版）或 windows-x64.zip（免安装版）下载。不需要 Git 或 GitHub 账号。', 'small')
add('3. 安装版：双击 setup.exe 按向导安装。免安装版：右键 ZIP → 全部解压，运行其中的 fpdf.exe。Source code 是开发源码，不能直接当阅读器运行。手册与校验文件也在 Assets。', 'small')
add('选择一种分发方式', 'h2')
table([
    ['文件', '用途'],
    [f'fpdf-{VERSION}-windows-x64-setup.exe', '标准安装程序：为当前用户安装，创建开始菜单快捷方式，支持卸载。'],
    [f'fpdf-{VERSION}-windows-x64.zip', '免安装压缩包：完整解压后运行其中的 fpdf.exe。'],
    ['SHA256SUMS.txt', '校验下载文件是否完整；PowerShell 使用 Get-FileHash -Algorithm SHA256。'],
])
add('发布包自带独立 PDFium 和 AI 所需的 Node.js，无需安装 Rust、WPS 或开发工具。请保留 pdfium.dll、ai/ 及许可证文件，不能只复制 fpdf.exe。')
add('打开、定位与连续阅读', 'h2')
add('点击“打开文件”、按 Ctrl+O，或将 PDF 拖进窗口。加密文档按提示输入密码。默认采用连续阅读，滚动时底部页码与侧栏高亮同步；可切换“单页阅读”。')
add('通过上一页、下一页、首页、末页，或底部页码输入框跳转。点击缩略图、左侧书签及正文目录中的内部链接也能定位；“后退”或 Alt+左箭头返回原页和滚动位置，Alt+右箭头前进。未嵌入链接的普通文字或扫描目录不自动跳转。')
add('缩放与多页排列', 'h2')
add('顶部和底部的缩小 / 放大按钮均可使用。底部百分比支持输入 10%-500% 并按 Enter，也可拖动比例滑块或按住 Ctrl 滚动鼠标滚轮。')
add('“实际大小”为 100%；“适合宽度”根据窗口宽度调整，“适合页面”完整显示当前页。“顺时针旋转”每次旋转 90 度。')
add('连续模式下缩小到合适比例，会按可用宽度自动排列为每行 2、4、8 页；放大后回到每行 1 页。点击小页面可选为当前页，红框显示选中页。单页模式始终只显示一页。')
add('滚动与当前页文本', 'h2')
add('Ctrl+F 或“全文搜索”在整个 PDF 中查找，右侧预览命中片段、页码和进度；点击结果跳转，上一个 / 下一个依次定位。支持区分大小写与全词匹配，页面黄色为匹配、橙色为当前结果。“文本面板”仍可复制当前页文字。')

section(3, '文档目录与阅读外观')
add('目录页码、折叠与跳转', 'h2')
add('左侧切换到“文档目录”。目录来自 PDF 自带书签；名称后显示目标页码，并用 * 引导对齐。页码从 1 开始计数，所有层级统一右对齐；无跳转目标的分组不显示页码。')
add('点击左侧三角形可折叠或展开子目录，也可选择“展开全部 / 折叠全部”。点击名称或页码即可跳转。长标题会换行，悬停可查看完整名称；星号与显示页码不属于书签名称。')
add('标签页与独立窗口', 'h2')
add('顶部 ＋ / Ctrl+T 新建标签；打开文件可多选，拖入 PDF 也会添加标签。点击切换，× / Ctrl+W 关闭，Ctrl+Tab / Ctrl+Shift+Tab 切换前后标签；各自保留页码、缩放、搜索、AI 和高亮撤销。Ctrl+N 新建独立窗口；“在新窗口打开 PDF…”保留。保存期间可切换，关闭前等待保存完成。')
add('双击名称，直接修改', 'h2')
add('双击目录名称，在原位置输入新名称。按 Enter 或点击其他位置保存到原 PDF；按 Esc 取消。保存名称不会重新加载页面或打断阅读位置。保存失败会保留输入并显示原因。')
add('需要改动层级或页码时，点击“编辑目录”。可新增章节 / 子章节、设置跳转页码、升降级、上下移动、删除及撤销。选择“保存到原 PDF”或“另存为…”，重新打开仍然生效。')
add('目录编辑会写入 PDF，重要文档可优先另存副本。源文件被其他程序修改时会停止保存；加密或含数字签名的 PDF 暂不支持目录编辑。', 'small')
add('阅读背景与自定义 RGB', 'h2')
table([
    ['设置', '操作'],
    ['默认 / 夜间 / 护眼 / 羊皮纸', '从顶部背景菜单选择；底部也提供“背景”和“护眼”入口。'],
    ['自定义 RGB', '打开“自定义 RGB…”，点击调色盘选色，拖动亮度条；R/G/B 数值同步，也可手动输入 0-255。'],
    ['确定 / 取消', '选色后立即预览；“确定”保留，“取消”恢复进入窗口前的颜色。'],
])
add('背景和文字颜色只改变阅读显示，不修改 PDF。主题在重启后保留，设置位于 %APPDATA%/fpdf/reading-theme.txt。')
add('高亮与下划线标注', 'h2')
add('先选文字再点“高亮标注”或“下划线”，也可先开工具再连续拖选；Esc 退出，Ctrl+Shift+U 切换下划线。两者分别保留颜色，当前工具的“标注颜色 → 自定义颜色…”提供调色盘、亮度和 RGB。Ctrl+Z 撤销，Ctrl+Y / Ctrl+Shift+Z 重做；后台保存期间仍可操作。两种标注均写入标准 PDF，重开保留，撤销历史重新开始。输入框内 Ctrl 操作仍编辑文字。')

section(4, 'AI 配置与文档问答')
add('先配置你自己的接口', 'h2')
add('AI 使用兼容 OpenAI 的接口。发布包不包含作者的接口地址、API 密钥或账号；下面仅为占位示例，请替换成你自己的配置。')
table([
    ['agent.txt（两行）', '填写内容'],
    ['第一行', 'https://your-api.example/v1'],
    ['第二行', 'YOUR_API_KEY'],
])
add('推荐直接点击 AI 面板的“API 配置”，填写接口基础地址和密钥，再点“保存并连接”。密钥默认隐藏显示，配置保存在 %APPDATA%/fpdf/agent.txt；重启后仍可用。即使 AI 正在工作，也能打开配置。')
add('无需先打开 PDF，即可点击“连接”测试接口、获取模型列表；“保存并连接”同样可在未打开文档时使用。打开 PDF 后再进行总结、选区翻译或文档问答。', 'small')
add('已有两行配置文件可点击“导入配置”；导入窗口在后台打开，不会卡住阅读界面。也可复制 agent.example.txt 为 agent.txt 后编辑，或用环境变量 FPDF_AGENT_CONFIG 指定配置。', 'small')
add('保留 ai/ 文件夹。点击顶部“AI 辅助阅读”，再点击“连接”。默认模型为 GLM-5.3-Flash；连接后可从接口返回的列表中选择其他模型。接口列出某模型不代表该账号一定能调用，失败时请检查自己的服务权限。')
add('全文搜索与 AI 可以同时打开，分别调整宽度或关闭；点击搜索结果不会中断正在生成的回答。窗口较窄时，两侧栏会限制宽度，为正文保留阅读区域。', 'small')
add('总结、提问与引用跳转', 'h2')
add('问题输入框直接显示，无需先翻译。选中文字后点击工具栏或右键中的“针对选区提问”，输入问题后发送；未连接时可“连接并提问”。提问会结合选区、附近上下文与页码。')
add('点击“总结当前页”快速获取要点；“概览文档”按目录选择代表页面。输入问题后点击“发送问题”，或按 Ctrl+Enter。回答逐步显示，可继续追问。')
add('回答下方的“第 N 页”按钮可跳转到引用页面。切换文档或模型会结束旧会话；概览会注明已读范围，不能替代全文逐页审阅。')
add('数据与读取范围', 'h2')
add('普通 PDF 阅读在本机完成。连接时只获取模型列表；翻译或提问时，问题、文档名称、目录、选中文字和相关页面文本会发送到你配置的服务。请根据文档内容选择是否使用 AI。')
add('Agent 只提供 PDF 文本读取工具，每次最多读取 5 页；长文本及会话存在长度限制，截断时会提示。扫描页没有文字时需要 OCR，目前不会识别图片中的文字。')

section(5, '选区翻译：单次与长开')
add('双击选中一个单词；按住左键拖动可选择同一页的任意文字及多行内容，选区按文字行实时高亮。拖到行尾空白处会吸附至该行文字。')
screenshot('fpdf-ai-preview.png', 430)
add('实际界面：两个单次按钮仍保留，旁边的“长期开启”勾选框独立控制自动翻译。', 'caption')
table([
    ['方式', '使用方法'],
    ['单次翻译', '选中后点“结合上下文翻译”；只处理本次选区，不改变长开状态。'],
    ['单次翻译并详解', '先给译文，再解释术语、指代、难句与含义；同样只执行本次操作。'],
    ['长期开启', '勾选对应按钮旁的开关。之后每次选词或拖选并松手，自动翻译；两种长开模式互斥。'],
])
add('连接和生成时显示“翻译进行中…”及旋转提示，译文流式更新，无需额外提问或点击发送。新选区会替换旧任务；“停止本次”仅停止当前任务，取消勾选才关闭长开模式。')
add('长开状态在本次运行中保持，切换文档 / 清空会话仍保留。支持中文、英语、日语、韩语目标语言。右键菜单也提供单次及长开操作。选区最多 8,000 字符，前后文各最多 1,500 字符；暂不支持跨页拖选。', 'small')

section(6, '快捷键与常见问题')
table([
    ['快捷键', '功能'],
    ['Ctrl+O / Ctrl+F', '打开 PDF / 全 PDF 查找（Ctrl+N 新窗口）'],
    ['Ctrl+F2 / Ctrl+Shift+U', '切换高亮 / 下划线；选中文字后可立即标注'],
    ['PageUp / PageDown', '上一页 / 下一页'],
    ['Ctrl+Home / Ctrl+End', '首页 / 末页'],
    ['Alt+Left / Alt+Right', '后退 / 前进'],
    ['Ctrl+0 / Ctrl+1', '适合宽度 / 实际大小'],
    ['Ctrl++ / Ctrl+-', '放大 / 缩小（也可 Ctrl+滚轮）'],
    ['Ctrl+Enter', '在文档问答输入框中发送问题'],
    ['Ctrl+Z / Ctrl+Y', '撤销 / 重做高亮；输入框内撤销 / 重做文字'],
])
add('常见问题', 'h2')
table([
    ['现象', '处理方法'],
    ['提示找不到 pdfium.dll', '完整安装或重新解压整个发布包，不要只复制 fpdf.exe。'],
    ['AI 无法启动 / 连接失败', '保留 ai/ 文件夹，检查 agent.txt 两行格式、地址可达性、密钥和模型权限。'],
    ['无法选择或翻译文字', '扫描页需要 OCR；从文字处开始选取，同一页内拖选。'],
    ['目录不能保存', '检查文件写权限、外部修改、密码与数字签名；可尝试另存副本。'],
])
add('确认卸载与清理', 'h2')
add('从“文件 → 卸载 fpdf（清除全部配置）”进入后，确认才会卸载；取消或按 Esc 返回。确认后关闭该安装的所有窗口，清理程序、AI 配置、主题、缓存与日志，保留 PDF。开始菜单及 Windows 设置也可卸载。免安装版退出后删除程序目录；配置在 %APPDATA%/fpdf。', 'small')
add('当前边界与开源信息', 'h2')
add('暂未提供 OCR、跨页拖选、附注文本编辑、页面内容编辑或打印。单文件最多 512 MiB。全文查找每页最多显示 2,000 条、全篇 20,000 条，达到上限会提示。复杂 PDF 的兼容性仍需结合实际文档判断。')
add('应用界面与逻辑由 Rust 实现，PDF 解析和渲染使用独立 PDFium；AI 使用 pi agent，不依赖 WPS 私有 DLL。应用代码采用 MIT 许可证，第三方条款随包保留。', 'small')
add('<link href="https://github.com/libo-libo/FPDF" color="#DA3336">源码与 README：github.com/libo-libo/FPDF</link><br/><link href="https://github.com/libo-libo/FPDF/releases" color="#DA3336">下载与版本记录：github.com/libo-libo/FPDF/releases</link>', 'small')

def page_frame(canvas, doc):
    canvas.saveState()
    canvas.setFillColor(RED)
    canvas.rect(0, A4[1]-8, A4[0], 8, fill=1, stroke=0)
    canvas.setFont('Guide', 9)
    canvas.setFillColor(MUTED)
    canvas.drawString(50, A4[1]-34, f'fpdf {VERSION}  /  中文使用说明')
    canvas.setStrokeColor(colors.HexColor('#E1E5EA'))
    canvas.line(50, 39, A4[0]-50, 39)
    canvas.drawString(50, 24, 'libo-libo/FPDF  ·  Windows x64')
    canvas.drawRightString(A4[0]-50, 24, f'{doc.page} / 6')
    if doc.page <= len(HEADINGS):
        canvas.bookmarkPage(f'section-{doc.page}')
        canvas.addOutlineEntry(HEADINGS[doc.page-1], f'section-{doc.page}', 0)
    canvas.restoreState()

doc = SimpleDocTemplate(str(OUTPUT), pagesize=A4, leftMargin=50, rightMargin=50, topMargin=55, bottomMargin=51,
                        title=f'fpdf {VERSION} 中文使用说明', author='fpdf contributors', subject='PDF 阅读、目录编辑、AI 问答与选区翻译')
doc.build(story, onFirstPage=page_frame, onLaterPages=page_frame)
print(OUTPUT)
