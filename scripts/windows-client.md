# Bong Windows Client · Quick Notes

## 常规流程

1. 启动服务端 / Agent
   在 WSL 中运行：`bash scripts/start.sh`

2. 同步 Bong 客户端 mod
   在 WSL 中运行：`bash scripts/windows-client.sh --sync-only`

3. 打开 Windows 启动器
   双击：`D:\Minecraft\Open-Bong-HMCL.bat`

4. 在 HMCL 中使用这个实例目录
   `D:\Minecraft\.minecraft\Fabric_Bang_Test`

5. 目标版本
   `1.20.1-Fabric`

6. 进入游戏后连接地址
   `localhost:25565`

7. 如果更新了 client 代码
   再次执行：`bash scripts/windows-client.sh --sync-only`

## 一键同步并打开 HMCL

合并步骤 2 + 3：

```bash
bash scripts/windows-client.sh --launch
```

该命令会先 `./gradlew build` → 拷贝最新 jar 到 `mods/` → 调用
`cmd.exe /c start "" "D:\Minecraft\Open-Bong-HMCL.bat"` 把启动器拉起来。
后续仍在 HMCL 里选 `Fabric_Bang_Test` 实例进游戏即可。
