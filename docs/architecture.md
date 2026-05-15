# Axum Template 架构分析

## 应用层 (app/)

- **controller/**: Axum handler，接收请求、调用 service、返回响应
- **dto/**: 请求/响应数据传输对象
- **helper/**: 提取器（`JwtCtx`、`SessionCtx`、`AppJson` 等）和工具函数
- **middleware/**: HTTP 中间件
- **router.rs**: 路由配置和 OpenAPI 集成
- **serve.rs**: 服务器启动
- **state.rs**: `AppState` 持有 config、db、services
- **error.rs**: `AppError` 的 `IntoResponse` 实现

## 领域层 (domain/)

- **db/**: 数据库连接初始化
- **model/**: Toasty 模型定义
- **service/**: 业务逻辑服务（user、role、permission、auth、session、token）

## 基础设施层

- **config/**: 配置管理（schema、manager、env、meta、paths）
- **error.rs**: `AppError`、`ErrorKind`、`Result` 类型
- **cli/**: 命令行工具（init、create-superuser、role/permission 管理）
- **util/**: 工具函数（密码哈希等）

## 数据流

```
HTTP 请求 → 中间件 → Controller → Service → Toasty ORM → DB → 响应
```

Controller 通过 `state.srv().xxx` 访问 Services，Services 持有 `toasty::Db`。

## 认证

两种机制共存，通过 extractor 选择：

- **`SessionCtx`**: 读 `session_token` cookie → 查 `sessions` 表 → 自动延长 TTL
- **`JwtCtx`**: 读 `Authorization: Bearer <token>` → 解码 JWT

## 技术栈

- **Web 框架**: Axum + Tokio
- **ORM**: Toasty (PostgreSQL)
- **认证**: Session (cookie) + JWT (`jsonwebtoken`)
- **文档**: utoipa + Swagger/Scalar
- **验证**: validator
- **序列化**: serde
