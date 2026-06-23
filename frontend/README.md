# SublinkX RS Frontend

Vue 3 + TypeScript + Vite 管理后台，当前已经接上：

- JWT 登录
- 节点管理
- 订阅管理
- `xray` / `mihomo` 导出链接展示

开发：

```bash
npm install
npm run dev
```

默认使用同源 `/api/`。本地开发如果后端单独运行在 8080，可通过 `.env` 配置：

```bash
VITE_API_BASE_URL=http://127.0.0.1:8080
```
