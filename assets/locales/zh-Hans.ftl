# Misc
-program-name = 派途

# Settings
settings-enable-romaji-search = 启用日语罗马字搜索
settings-show-performance-stats = 显示性能信息
settings-enable-autosave = 启用自动保存
settings-autosave-interval = 自动保存间隔（分钟）
settings-enable-developer-mode = 启用开发者模式

# Side panel
side-panel-edit = 编辑
side-panel-details = 信息
side-panel-export = 导出

# Fallback messages
side-panel-edit-fallback-1 = 当前选中的页面尚未实现「{side-panel-edit}」功能栏
side-panel-edit-fallback-2 = 等俺实现先，当然也请请您在 GitHub 上开个 issue 反馈一下这个问题！
side-panel-details-fallback-1 = 当前选中的页面尚未实现「{side-panel-details}」功能栏
side-panel-details-fallback-2 = {side-panel-edit-fallback-2}
side-panel-export-fallback-1 = 当前选中的页面尚未实现「{side-panel-export}」功能栏
side-panel-export-fallback-2 = 太可恶了这个作者怎么什么都没有做

# Tabs
# Start tab
tab-start = 开始
tab-start-version = 版本：{$version}
tab-start-revision = 开发号：{$revision}
tab-start-description = A high-performance transport timetable diagramming and analysis tool built with egui and Bevy.
# Settings tab
tab-settings = 设置
# Diagram tab
tab-diagram = 运行图
tab-diagram-export-typst = 导出为 Typst 文档
tab-diagram-export-typst-desc = 将当前运行图导出为 Typst 文档。导出的文档可在文本编辑器中进一步编辑。
tab-diagram-export-typst-output = Typst 输出长度：{$bytes} 字节
# Graph tab
tab-graph = 线路网
tab-graph-new-displayed-line = 新建基线
tab-graph-new-displayed-line-desc = 新建基线。基线可用于显示运行图
tab-graph-auto-arrange = 自动整理线路网
tab-graph-auto-arrange-desc = 使用力导向布局算法自动整理线路网。调整下方参数以改变布局效果。
tab-graph-auto-arrange-iterations = 迭代次数
tab-graph-arrange-via-osm = 通过 OSM 整理
tab-graph-arrange-button = 整理
# tip: use local examples of area names
tab-graph-arrange-via-osm-desc = 利用在线资源整理当前线路网。本功能使用 OpenStreetMap 数据，点击「{tab-graph-arrange-button}」即表示同意 OpenStreetMap 的使用条款。
    可以填写一个区域名称以限制查询范围（如：北京市、温州市）。
tab-graph-arrange-via-osm-terms = 使用条款
tab-graph-osm-area-name = 过滤区域：
tab-graph-animation = 动画控制
tab-graph-animation-desc = 控制动态运行图动画。

# new lines desc
new-displayed-line = 新基线

# general
copy-to-clipboard = 复制到剪贴板
done = 完成
export = 导出

# RW data
oud2-default-line = OUD2 默认运行线
oud2-unnamed-line = 未命名路线 {$number}
oud2-unnamed-station = 未命名车站 {$number}
oud2-unnamed-diagram = 未命名运行图 {$number}
oud2-unnamed-train = 未命名列车 {$number}
