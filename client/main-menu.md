# 主菜单第一版

本地配置为 Minecraft 实例的 `config/bong-client.json`，开发运行时位于 `client/run/config/bong-client.json`。
首次启动创建空地址，保留部署者后续修改；只通过配置指定目标，界面不提供单人游戏或服务器列表。

```json
{
  "serverAddress": "127.0.0.1:25565",
  "motion": true,
  "ambience": true
}
```

`serverAddress` 支持域名、`host:port` 和 `[IPv6]:port`。每次点击入世重新读取配置。
`motion: false` 关闭视差、尘埃和推近；`ambience: false` 关闭菜单环境声。音量同时受原版环境音量控制。
设置页面复用原版功能；连接、取消、握手和地形加载由 Minecraft 持有。
本端所有游玩均在配置的服务器上进行，入世和重试直接连接，不采用原版标题页禁用多人游戏按钮的检查。
断线说明完整保留，超出窗口时可滚动查看；重试和返回按钮保持在文本区域之外。

## 美术来源

- 场景：`gpt-image-2` 母图及图生图派生，原始产物与 prompt 在工作区 `local_images/main-menu-r1/`。
- 客户端资源：`src/main/resources/assets/bong/textures/gui/main_menu/`。
- 字标：Ma Shan Zheng，SIL Open Font License；随包保留 `assets/bong/font/menu-title-ofl.txt`。
- 环境音：原版随包的 soul sand valley 环境声，低音量播放。
- 第一轮实现，尚未宣称完成三轮美术打磨。灵龛已从本菜单设计移除。

## 验证

```bash
JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ../scripts/build-token.sh gradle test build
BONG_UI_PREVIEW_CONFIG="$PWD/main-menu-preview.json" JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ../scripts/build-token.sh gradle runClientUiPreview
```

截图写入 `build/main-menu-preview/`，覆盖 4:3、16:9、超宽和大 GUI 缩放。

第一版仅新增配置文件不被加载过程覆盖的持久化测试；既有 UI 清单同步登记新 Screen。
真实客户端已验证设置返回、空地址、连接拒绝、重试、握手取消、重复返回和退出。
这些交互通过临时调试脚本执行，不向产品加入测试入口。尚未连接实际游戏服务器验证完整入世链路。

断线说明修复已通过 Java 17 的 `gradle test build`（5148 单测、3 GameTest）。
真实客户端在 854×480 窗口验证了百行中文说明的末尾可达、滚轮、键盘、滚动条拖动、短说明、返回和重试。
该次运行使用 OpenAL null 驱动避开 WSL 音频设备重连；窗口缩放未实际生效，断线页的多尺寸截图验证尚未完成。
