# Misc
-program-name = 派途
# Settings
settings-enable-romaji-search = 启用日语罗马字搜索
settings-show-performance-stats = 显示性能信息
settings-enable-autosave = 启用自动保存
settings-autosave-interval = 自动保存间隔（分钟）
settings-enable-developer-mode = 启用开发者模式
settings-preferences = 首选项
settings-dark-mode = 深色模式
settings-language = 语言
settings-project-settings = 项目设置
# Side panel
side-panel-edit = 编辑
side-panel-details = 信息
side-panel-export = 导出
# Fallback messages
side-panel-edit-fallback-1 = 当前选中的页面尚未实现「{ side-panel-edit }」功能栏
side-panel-edit-fallback-2 = 等俺实现先，当然也请请您在 GitHub 上开个 issue 反馈一下这个问题！
side-panel-details-fallback-1 = 当前选中的页面尚未实现「{ side-panel-details }」功能栏
side-panel-details-fallback-2 = { side-panel-edit-fallback-2 }
side-panel-export-fallback-1 = 当前选中的页面尚未实现「{ side-panel-export }」功能栏
side-panel-export-fallback-2 = 太可恶了这个作者怎么什么都没有做
# Tabs
# Start tab
tab-start = 开始
tab-start-merge-stations-by-name = 按名称合并车站
tab-start-amount-vehicles = 车辆数量：
tab-start-amount-trips = 车次数量：
tab-start-amount-stations = 车站数量：
tab-start-amount-platforms = 站台数量：
tab-start-amount-intervals = 区间数量：
tab-start-version = 版本：{ $version }
tab-start-revision = 开发号：{ $revision }
tab-start-description = 使用 egui 与 Bevy 打造的高性能运输时刻表编辑与分析工具
# Settings tab
tab-settings = 设置
# Diagram tab
tab-diagram = 运行图
tab-diagram-export-typst-diagram-output = Typst 输出长度：{ $bytes } 字节
# Graph tab
tab-graph = 线路网
tab-graph-new-displayed-line = 新建基线
tab-graph-new-displayed-line-desc = 新建基线。基线可用于显示运行图
tab-graph-auto-arrange = 自动整理线路网
tab-graph-auto-arrange-desc = 使用力导向布局算法自动整理线路网。调整下方参数以改变布局效果。
tab-graph-auto-arrange-iterations = 迭代次数
tab-graph-arrange-via-osm = 通过 OSM 整理
tab-graph-arrange-button = 整理
tab-graph-arrange-mode-force = 力导向
tab-graph-arrange-mode-osm = OSM
tab-graph-arrange-progress = 整理（{ $mode }）进度：{ $finished }/{ $total } | 重试排队：{ $queued_retry }
# tip: use local examples of area names
tab-graph-arrange-via-osm-desc =
    利用在线资源整理当前线路网。本功能使用 OpenStreetMap 数据，点击「{ tab-graph-arrange-button }」即表示同意 OpenStreetMap 的使用条款。
    可以填写一个区域名称以限制查询范围（如：北京市、温州市）。
tab-graph-arrange-via-osm-terms = 使用条款
tab-graph-osm-area-name = 过滤区域：
tab-graph-animation = 动画控制
tab-graph-animation-desc = 控制动态运行图动画。
tab-graph-underlay-none = 无
tab-graph-underlay-openstreetmap = OpenStreetMap
tab-graph-underlay-amap = 高德地图（AutoNavi）
tab-graph-underlay-chiriin = 日本地理院地图
# Trip tab
trip-table-station = 车站
trip-table-arrival = 到达
trip-table-departure = 发车
# new lines desc
new-displayed-line = 新基线
# general
copy-to-clipboard = 复制到剪贴板
done = 完成
export = 导出
# RW data
oud2-default-line = OUD2 默认运行线
oud2-unnamed-line = 未命名路线 { $number }
oud2-unnamed-station = 未命名车站 { $number }
oud2-unnamed-diagram = 未命名运行图 { $number }
oud2-unnamed-train = 未命名列车 { $number }
# Colours
colour-red = 红色
colour-orange = 橙色
colour-amber = 琥珀
colour-yellow = 黄色
colour-lime = 酸橙
colour-green = 绿色
colour-emerald = 祖母绿
colour-teal = 蓝绿
colour-cyan = 青色
colour-sky = 天蓝
colour-blue = 靛蓝
colour-indigo = 紫色
colour-violet = 紫罗兰
colour-purple = 紫色
colour-fuchsia = 紫红
colour-pink = 粉色
colour-rose = 玫瑰
colour-slate = 板岩灰
colour-gray = 灰色
colour-zinc = 锌色
colour-neutral = 中性
colour-stone = 石色
# read files
read-file-prompt = 读取 { $name }…
read-file-title = 读取 { $name } 文件
read-file-filetype = { $name } 文件
tab-diagram-save-typst-module = 保存 Typst 模块
tab-diagram-save-typst-module-desc = 渲染 JSON 数据必须使用 Typst 模块
tab-diagram-export-json-data = 将运行图导出为 JSON
tab-diagram-export-json-data-desc = 将当前运行图导出为 JSON
tab-diagram-export-typst-timetable = 输出为时刻表（Typst）
tab-diagram-export-typst-timetable-desc = 将当前运行图之时刻表导出为 Typst 时刻表。导出的时刻表可根据需要在任意文本编辑器内进一步编辑
tab-diagram-export-json-timetable = 导出为时刻表（JSON）
tab-diagram-export-json-timetable-desc = 将当前运行图之时刻表导出为 JSON 文件。导出的时刻表可根据需要使用其他工具处理
menu-import-url-heading = 从网址导入
menu-import-url-desc = 从互联网下载文件并导入派途
menu-url-label = 网址：
menu-download-and-import = 下载并导入
menu-route-timetable = 路线时刻表
menu-new-route = 新路线
menu-priority-graph = 优先等级图
menu-diagrams = 运行图
menu-trips = 车次
menu-new-trip = 新车次
menu-text = 文本
menu-new-text-message = 新文本
menu-new-message = 新消息
menu-project-remarks = 项目备注
menu-nothing-focused = 尚未聚焦任何面板
menu-more = 更多…
menu-fullscreen = 全屏幕
menu-import-url-prompt = 从网址导入…
menu-save = 保存…
menu-read = 读取…
menu-load-save = 加载存档
menu-paiagram-savefiles = 派途存档
menu-save-ron = 保存 RON…
menu-read-ron = 读取 RON…
menu-load-ron-files = 加载 RON 文件
menu-ron-files = RON 文件
menu-about = 关于
menu-documentation = 文档
menu-legal = 法律信息
menu-sync-system-clock = 与系统时钟同步
menu-maximized-view = 最大化视口
menu-undo = 撤销
menu-redo = 重做
tab-classes = 种别
classes-name = 种别名
classes-count = 数量
classes-color = 颜色
diagram-export-oudia = 导出至 OuDia
diagram-use-global-timer = 使用全局计时器
diagram-create-new-trip-scratch = 从零新建车次
diagram-create-new-trip = 新建车次
diagram-complete = 完成
diagram-find-route-between = 在两站间查找乘车方案…
diagram-arrival-time = 到达时间：
diagram-already-editing = 正在编辑中…
graph-create-new-route = 新建路线
graph-new-station = 新车站
tab-route-timetable = 路线时刻表
route-timetable-sort-entries = 排序项目
route-timetable-stations = 车站
tab-priority-graph = 优先等级图
settings-developer-mode = 开发者模式
settings-antialiasing-options = 抗锯齿选项
settings-off = 关
settings-on = 开
settings-lod-mode = 细节层次控制
settings-lod-2x = 2×
settings-lod-4x = 4×
tab-station = 车站
station-include-non-stop = 包含不停站车次
tab-text = 文本信息
text-markdown-hint = 可在此处使用 Markdown
tab-trip = 车次
widget-at = {"\uE65C"} 定时
widget-for = {"\uE12A"} 用时
widget-flexible = {"\uE6DE"} 不定时
widget-non-stop = {"\uE06C"} 不停车
