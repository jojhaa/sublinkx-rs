# Clash / Mihomo 国内直连排查

## 内置导出的修正

- Clash 的远程 `.list` 规则集显式声明 `format: text`，不再按默认 YAML 格式读取。
- 国内直连 DNS 增加 `223.5.5.5`、`119.29.29.29`，避免只依赖 HTTPS DNS。普通 DNS 未加密，仅适用于接受向这些解析服务查询国内直连域名的环境。
- Mihomo 的国内域名规则集与 `.cn` 使用国内 DNS，直连重新解析不再跟随国外域名策略；国外默认 DNS 仍经代理查询。
- 国内域名加入 Fake-IP 排除，并增加 `.cn` 直连兜底；国内 IP 规则允许解析真实地址后匹配，不再因 `no-resolve` 跳过。
- 旧的同名系统内置模板在导出时同步应用修正，无需手工删除模板或重建订阅。自定义和上游透传模板不强制修改。

## 客户端检查

1. 服务端发布修复后，更新订阅并重新加载配置，确认使用规则模式，不是全局代理或全局直连。
2. 检查客户端是否开启 DNS 覆写。若使用 FlClash，客户端 DNS 覆写可能替换订阅中的整个 DNS 段；使用订阅 DNS 后再复测。不同客户端版本的菜单名称可能不同。
3. 查看实际运行配置，而不只是下载文件。确认国内请求命中 `DIRECT`，国内 DNS 可达，日志中没有规则集解析、DNS 超时或连接错误。
4. 若仍失败，提供客户端准确名称、版本、一个失败域名和对应错误日志。请隐藏订阅 Token、节点凭据和完整订阅地址。

设备所在网络可能限制直连或 DNS；配置修正不能绕过这些限制。Clash 导出仍使用远程规则集，首次下载依赖网络；Mihomo 内置规则继续随文件提供。旧版 Clash 核心的格式兼容性需要单独验收。

## 本地复验

Windows PowerShell 7，在项目 `backend` 目录执行：

```powershell
cargo test --locked --offline
cargo test --locked --offline generate_routing_smoke_exports -- --ignored
```

第二条命令将合成节点的导出文件写入 `output/routing-smoke`，不读取业务数据库。随后可使用本机已安装的 Mihomo 核心执行 `-t -d <隔离目录> -f <导出文件>`。配置校验不代表真实节点、国内外访问或远程规则下载均已验收。

## 协议依据

- [Mihomo 规则集格式](https://wiki.metacubex.one/config/rule-providers/)
- [Mihomo DNS 配置](https://wiki.metacubex.one/config/dns/)
- [FlClash 配置组装源码](https://github.com/chen08209/FlClash/blob/main/lib/common/task.dart)

本次没有引入第三方代码、依赖或素材，仅依据官方配置语义修正项目生成内容。
