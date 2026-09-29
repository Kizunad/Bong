import { copyFileSync } from "node:fs";

// 发布包携带共享目录，不依赖 Agent 启动时的工作目录。
copyFileSync(
  new URL("../../../../shared/botany/plants.json", import.meta.url),
  new URL("../dist/plants.json", import.meta.url),
);
