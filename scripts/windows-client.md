# Bong Windows Native Client · Quick Notes

## 常规流程

1. 启动服务端 / Agent
   在 WSL 中启动服务端。

2. 同步 Bong 客户端 mod
   在 WSL 中运行：`bash scripts/windows-client.sh --sync-only`

3. 启动 Windows Native Fabric 客户端
   在 PowerShell 中运行：
   `powershell -ExecutionPolicy Bypass -File D:\Minecraft\.minecraft\Fabric_Bang_Test\bong-native\forge-interact-launch.ps1`

4. Native 使用这个实例目录
   `D:\Minecraft\.minecraft\Fabric_Bang_Test`

5. 目标版本
   `1.20.1-Fabric`

6. 进入游戏后连接地址
   `localhost:25565`

7. 如果更新了 client 代码
   再次执行：`bash scripts/windows-client.sh --sync-only`

## Windows Native 启动链

`scripts/windows-client.sh` 只用 Java 17 构建并同步 jar，不再启动 HMCL。同步完成后，
通过 `bong-native/forge-interact-launch.ps1` 直接读取 `launch.args` 启动 Fabric；该参数文件
已经包含 `127.0.0.1:25565` 的 quick play 连接地址。

Java 17 在 Windows 上按系统代码页读取 `@argfile`。`launch.args` 保持 UTF-8，
原生启动脚本先用 `Encoding.UTF8` 读取，再以 `Encoding.Default` 写入
`launch-native.args` 后启动；直接读取 UTF-8 文件会让中文离线用户名变成乱码，
导致实际登录名与服务端 OP 名单不一致。
