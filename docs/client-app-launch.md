# 订阅门户客户端唤起

门户的主操作使用客户端导入协议，不打开 HTTP 下载页。浏览器无法可靠枚举已安装应用，也无法确认客户端是否成功保存订阅；界面不据此宣称安装或导入成功。

| 客户端 | 导入方式 |
| --- | --- |
| Mihomo / Clash 图形客户端 | `clash://install-config?url=...`，由系统注册的客户端处理 |
| sing-box | `sing-box://import-remote-profile?url=...#名称` |
| Shadowrocket | 保留 `shadowrocket://config/add/` 完整配置入口 |
| Surge | `surge:///install-config?url=...` |
| Loon | `loon://import?sub=...` |
| Surfboard | `surfboard:///install-config?url=...` |
| v2rayNG | Android 使用 `v2rayng://install-sub?url=...#名称`，传入 Mixed URI 节点订阅地址 |
| Quantumult X | 完整配置仅提供手动导入说明，不把完整配置误作远程节点资源 |
| v2rayN | 未确认当前客户端的网页唤起契约，提供手动导入说明 |

智能识别入口展示客户端选择器，选中的应用使用对应格式的链接。URL 参数编码保留订阅原有查询参数；不自动跳转应用商店，不连续试探多个应用，不在地址、日志或第三方检测服务中额外传送订阅信息。

## 官方协议来源

以下只用于核对协议契约，没有复制第三方实现或引入依赖。现有项目许可证和 NOTICE 继续保留。

- [Clash Verge Rev](https://www.clashverge.dev/guide/url_schemes.html)
- [sing-box](https://sing-box.sagernet.org/zh/clients/general/)
- [Surge](https://manual.nssurge.com/tools/url-scheme.html)
- [Loon](https://nsloon.app/en/docs/Scheme/)
- [Surfboard](https://getsurfboard.com/docs/deeplink/)
- [v2rayNG 协议处理入口](https://github.com/2dust/v2rayNG/blob/master/V2rayNG/app/src/main/java/com/v2ray/ang/ui/UrlSchemeActivity.kt)
- [Quantumult X 协议说明](https://github.com/crossutility/Quantumult-X/blob/master/url-scheme.md)

核对日期：2026-09-09。客户端版本、浏览器或系统协议注册状态可能影响唤起；原生应用中的实际导入仍需实机验收。

## 本地检查

Windows PowerShell 7，前端目录：

```powershell
$ErrorActionPreference = 'Stop'
node scripts/check-portal-launch.mjs
if ($LASTEXITCODE -ne 0) { throw '协议检查失败' }
npm run build
if ($LASTEXITCODE -ne 0) { throw '前端构建失败' }
```

浏览器验证脚本 `frontend/scripts/check-portal-browser.js` 使用虚构门户响应，配合本地 5198 端口和 Playwright CLI `run-code --filename` 执行。它不会访问生产数据或安装真实客户端。
