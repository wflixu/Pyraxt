# Actix 到 Axum 迁移设计文档

## 1. 概述

### 1.1 项目背景

Axon (Axon) 是一个高性能的 Python Web 框架，使用 Rust 作为底层运行时。当前实现基于 Actix-web，本迁移计划旨在将其迁移到 Axum。

### 1.2 迁移动机

| 目标 | 评估 |
|------|------|
| 性能提升 | ❌ 不显著（Actix ~3.2M req/s, Axum ~2.8M req/s，差距 < 15%） |
| 生态兼容 | ✅ Tower 生态更丰富，更好的中间件支持 |
| 代码简洁 | ✅ 预计代码量减少 30-40% |
| 维护成本 | ✅ 编译更快，依赖更少 |
| 学习曲线 | ✅ Tower 模式更符合 Rust 标准 |

### 1.3 可行性结论

**技术上完全可行，但需要大量重写**。主要工作量在于：
- WebSocket 模块完全重写（Actor → Stream 模型）
- HTTP 服务器构建逻辑重构
- 类型系统适配

**Python 层无需修改** - PyO3 FFI 边界保持不变。

---

## 2. 架构对比

### 2.1 当前架构 (Actix)

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Python Layer                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │   Axon      │  │  SubRouter   │  │ WebSocket    │               │
│  │  (app.py)    │  │              │  │  API         │               │
│  └──────┬───────┘  └──────────────┘  └──────────────┘               │
│         │                                                            │
│         ▼                                                            │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │              PyO3 FFI Boundary                                │   │
│  │  Server, Request, Response, Headers, FunctionInfo...         │   │
│  └─────────────────┬────────────────────────────────────────────┘   │
│                    ▼                                                │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    Actix Runtime                              │   │
│  │  ┌────────────────────────────────────────────────────────┐   │   │
│  │  │  Actix-web HTTP Server (Actor 模型)                     │   │   │
│  │  │  ┌────────┬────────┬────────┬─────────────────────┐   │   │   │
│  │  │  │HttpRouter│ConstRouter│MiddlewareRouter│WebSocketRouter│   │   │
│  │  │  └────────┴────────┴────────┴─────────────────────┘   │   │   │
│  │  └────────────────────────────────────────────────────────┘   │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 目标架构 (Axum)

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Python Layer                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │   Axon      │  │  SubRouter   │  │ WebSocket    │               │
│  │  (app.py)    │  │              │  │  API         │               │
│  └──────┬───────┘  └──────────────┘  └──────────────┘               │
│         │                                                            │
│         ▼                                                            │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │              PyO3 FFI Boundary                                │   │
│  │  Server, Request, Response, Headers, FunctionInfo...         │   │
│  └─────────────────┬────────────────────────────────────────────┘   │
│                    ▼                                                │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    Axum Runtime                               │   │
│  │  ┌────────────────────────────────────────────────────────┐   │   │
│  │  │  Axum HTTP Server (Stream 模型)                         │   │   │
│  │  │  ┌────────┬────────┬────────┬─────────────────────┐   │   │   │
│  │  │  │HttpRouter│ConstRouter│MiddlewareRouter│WebSocketRouter│   │   │
│  │  │  └────────┴────────┴────────┴─────────────────────┘   │   │   │
│  │  └────────────────────────────────────────────────────────┘   │   │
│  │  ┌────────────────────────────────────────────────────────┐   │   │
│  │  │  Tower Middleware Stack                                 │   │   │
│  │  │  Trace, Limit, Cors, Timeout...                        │   │   │
│  │  └────────────────────────────────────────────────────────┘   │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. 核心差异分析

### 3.1 模型对比：Actor vs Stream

| 维度 | Actor 模型 (Actix) | Stream 模型 (Axum) |
|------|-------------------|-------------------|
| **抽象层次** | 高层抽象，内置消息系统 | 底层抽象，直接使用 Stream trait |
| **状态管理** | 每个 Actor 封装自己的状态 | 状态共享（Arc<Mutex>/Channel） |
| **消息传递** | 内置 Mailbox 系统 | 需要手动管理 channel |
| **生命周期** | Actor Context 管理 | 手动管理 Task 生命周期 |
| **错误处理** | Actor 监督机制 | 需要手动传播错误 |
| **学习曲线** | 陡峭 | 平缓 |
| **代码量** | 较少 | 较多 |
| **灵活性** | 受限 | 高 |
| **生态兼容** | 封闭 | 开放（Tower） |

### 3.2 HTTP 服务器构建

**Actix 当前实现:**
```rust
// src/server.rs:128-228
actix_web::rt::System::new().block_on(async move {
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(router.clone()))
            .app_data(web::Data::new(const_router.clone()))
            .default_service(web::route().to(index))
    })
    .keep_alive(KeepAlive::Os)
    .workers(workers)
    .listen(raw_socket.into())
    .unwrap()
    .run()
    .await
});
```

**Axum 目标实现:**
```rust
let listener = tokio::net::TcpListener::from_std(raw_socket)?;
axum::serve(listener, make_router())
    .await?;
```

### 3.3 WebSocket 处理

**Actix (Actor 模型):**
```rust
// src/websockets/mod.rs
pub struct WebSocketConnector {
    pub id: Uuid,
    pub registry_addr: Addr<WebSocketRegistry>,
}

impl Actor for WebSocketConnector {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.registry_addr.do_send(Connect { id: self.id });
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        self.registry_addr.do_send(Disconnect { id: self.id });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketConnector {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Text(text) => { /* ... */ }
            ws::Message::Close(_) => ctx.stop(),
        }
    }
}
```

**Axum (Stream 模型):**
```rust
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Connect(query_params): Connect,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        handle_socket(socket, query_params).await;
    })
}

async fn handle_socket(socket: WebSocket, query_params: QueryParams) {
    let (mut tx, mut rx) = socket.split();
    let (tx_send, mut rx_recv) = mpsc::channel(32);

    // 发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx_recv.recv().await {
            tx.send(msg).await.ok();
        }
    });

    // 接收任务
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = socket.next().await {
            match msg {
                Message::Text(text) => { /* ... */ }
                Message::Close(_) => break,
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

### 3.4 中间件系统

**Actix:**
```rust
// 在 handler 中手动执行中间件
for before_middleware in before_middlewares {
    request = execute_middleware_function(&request, &before_middleware).await?;
}
```

**Axum:**
```rust
use tower::Layer;
use tower_http::trace::TraceLayer;

let app = Router::new()
    .route("/", get(handler))
    .layer(TraceLayer::new_for_http())
    .layer(middleware::from_fn(custom_middleware));
```

---

## 4. 依赖变更

### 4.1 Cargo.toml 变更

**移除的依赖:**
```toml
actix = "0.13.4"
actix-web = "4.4.2"
actix-web-actors = "4.3.0"
actix-http = "3.3.1"
actix-files = "0.6.2"
actix-multipart = "0.6.1"
```

**新增的依赖:**
```toml
axum = { version = "0.7", features = ["ws", "multipart"] }
axum-core = "0.4"
tower = { version = "0.4", features = ["full"] }
tower-http = { version = "0.5", features = ["fs", "trace", "limit", "cors"] }
tokio-tungstenite = "0.21"
hyper = { version = "1.0", features = ["full"] }
hyper-util = { version = "0.1", features = ["full"] }
http-body-util = "0.1"
```

**保留的依赖:**
```toml
pyo3 = { version = "0.20.0", features = ["extension-module"] }
pyo3-asyncio = { version = "0.20.0", features = ["attributes", "tokio-runtime"] }
tokio = { version = "1", features = ["full"] }
socket2 = "0.5"
matchit = "0.7"
uuid = { version = "1.4", features = ["v4"] }
```

### 4.2 Feature Flag 设计

为支持渐进式迁移和回滚，设计以下 feature flags:

```toml
[features]
default = ["actix-runtime"]

# 当前 Actix 实现
actix-runtime = [
    "dep:actix",
    "dep:actix-web",
    "dep:actix-web-actors",
    "dep:actix-http",
    "dep:actix-files",
    "dep:actix-multipart",
]

# 新 Axum 实现
axum-runtime = [
    "dep:axum",
    "dep:axum-core",
    "dep:tower",
    "dep:tower-http",
    "dep:tokio-tungstenite",
    "dep:hyper",
    "dep:hyper-util",
    "dep:http-body-util",
]
```

**构建命令:**
```bash
# 默认构建 (Actix)
cargo build

# Axum 构建
cargo build --features axum-runtime --no-default-features
```

---

## 5. 模块迁移详细设计

### 5.1 可复用模块（无需修改）

以下模块可以几乎原样复用，仅需类型签名适配：

| 模块 | 路径 | 修改量 | 说明 |
|------|------|--------|------|
| HTTP Router | `src/routers/http_router.rs` | ~10% | 路由表管理逻辑不变 |
| Const Router | `src/routers/const_router.rs` | ~10% | 常量路由逻辑不变 |
| Middleware Router | `src/routers/middleware_router.rs` | ~10% | 中间件路由逻辑不变 |
| WebSocket Router | `src/routers/web_socket_router.rs` | ~10% | WS 路由表逻辑不变 |
| Shared Socket | `src/shared_socket.rs` | ~5% | Socket 管理完全可复用 |

### 5.2 需适配模块

#### 5.2.1 类型模块 - Response (`src/types/response.rs`)

**当前实现 (Actix):**
```rust
impl Responder for Response {
    type Body = BoxBody;

    fn respond_into_response(self) -> Response<BoxBody> {
        let mut builder = actix_web::HttpResponseBuilder::new(self.status_code);

        for (key, value) in self.headers.iter() {
            builder.append_header((key, value));
        }

        builder.body(self.body.to_string())
    }
}
```

**目标实现 (Axum):**
```rust
impl IntoResponse for Response {
    fn into_response(self) -> axum::response::Response {
        let mut builder = http::Response::builder()
            .status(self.status_code);

        for (key, value) in self.headers.iter() {
            builder = builder.header(key.as_str(), value.as_str());
        }

        let body = axum::body::Body::from(self.body.to_string());
        builder.body(body).unwrap().into_response()
    }
}
```

#### 5.2.2 类型模块 - Request (`src/types/request.rs`)

**当前实现 (Actix):**
```rust
impl Request {
    pub async fn from_actix_request(
        req: &HttpRequest,
        mut payload: web::Payload,
        global_headers: &Headers,
    ) -> Self {
        let headers = Headers::new(req.headers(), global_headers);
        let url = Url::new(req.connection_info(), req.query_string());

        let (body_type, body) = Self::extract_body(&mut payload).await;

        Request {
            path: req.path().to_string(),
            method: Self::extract_method(req.method()),
            headers,
            url,
            body,
            body_type,
        }
    }
}
```

**目标实现 (Axum):**
```rust
impl Request {
    pub async fn from_axum_request(
        req: &http::Request<axum::body::Body>,
        headers: &HeaderMap,
    ) -> Self {
        let headers = Headers::from_http_headers(headers);
        let url = Url::from_request_uri(req.uri());

        let (body_type, body) = Self::extract_body(req.into_body()).await;

        Request {
            path: req.uri().path().to_string(),
            method: Self::from_http_method(req.method()),
            headers,
            url,
            body,
            body_type,
        }
    }
}
```

#### 5.2.3 执行器模块 (`src/executors/mod.rs`)

**当前实现:**
```rust
pub async fn execute_http_function(
    function: &FunctionInfo,
    request: Request,
) -> Result<Response, Box<dyn std::error::Error>> {
    let function_output = function.handler.call1((request,))?;

    if function.is_async {
        let future = pyo3_asyncio::tokio::into_future(function_output)?;
        let response = pyo3_asyncio::tokio::run_until_complete(
            function.event_loop,
            future,
        ).await?;

        Ok(Response::from_pyresponse(response))
    } else {
        Ok(Response::from_pyresponse(function_output))
    }
}
```

**目标实现:**
```rust
// 核心逻辑不变，仅类型签名调整
pub async fn execute_http_function(
    function: &FunctionInfo,
    request: Request,
) -> Result<Response, Box<dyn std::error::Error>> {
    // ... 逻辑完全相同 ...
}
```

### 5.3 需重写模块

#### 5.3.1 Server 模块 (`src/server.rs`)

**当前实现关键部分:**
```rust
// 使用 Actix HttpServer
let server = HttpServer::new(move || {
    let mut app = App::new();

    // 静态文件服务
    for directory in directories.iter() {
        app = app.service(
            Files::new(&directory.route, &directory.directory_path)
                .index_file(&directory.index_file)
                .redirect_to_slash_directory()
        );
    }

    // 路由配置
    app = app
        .app_data(web::Data::new(router.clone()))
        .app_data(web::Data::new(const_router.clone()))
        .default_service(web::route().to(index));

    app
})
.keep_alive(KeepAlive::Os)
.workers(workers)
.listen(raw_socket.into())
.unwrap()
.run()
.await;
```

**目标实现:**
```rust
pub fn start(&mut self, py: Python, socket: &PyCell<SocketHeld>, workers: usize) -> PyResult<()> {
    pyo3_log::init();

    if STARTED.compare_exchange(false, true, SeqCst, Relaxed).is_err() {
        debug!("Axon is already running...");
        return Ok(());
    }

    let raw_socket = socket.try_borrow_mut()?.get_socket();

    // 创建 Tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .unwrap();

    let router = self.router.clone();
    const_router = self.const_router.clone();
    let middleware_router = self.middleware_router.clone();
    let web_socket_router = self.websocket_router.clone();

    // 构建 Axum Router
    let app = make_router(
        router,
        const_router,
        middleware_router,
        web_socket_router,
    );

    // 转换为 Axum 服务
    let listener = tokio::net::TcpListener::from_std(raw_socket)
        .map_err(|e| PyIOError::new_err(e.to_string()))?;

    runtime.block_on(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok(())
}
```

**Router 构建函数:**
```rust
fn make_router(
    router: Arc<HttpRouter>,
    const_router: Arc<ConstRouter>,
    middleware_router: Arc<MiddlewareRouter>,
    web_socket_router: Arc<WebSocketRouter>,
) -> Router {
    let mut app = Router::new();

    // 静态文件服务
    for directory in directories.iter() {
        app = app.nest_service(
            &directory.route,
            ServeDir::new(&directory.directory_path)
                .append_index_html_on_directories(true),
        );
    }

    // 主路由 handler
    app = app
        .route("/*path", web::route().to(index_handler));

    // WebSocket 路由
    for (endpoint, _) in web_socket_router.get_routes().iter() {
        app = app.route(endpoint, get(websocket_handler));
    }

    // 添加状态
    app = app.with_state(RouterState {
        router,
        const_router,
        middleware_router,
    });

    // 添加 Tower 中间件
    app = app
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(60)))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024)); // 2MB

    app
}
```

#### 5.3.2 WebSocket 模块 (`src/websockets/mod.rs`)

**完全重写设计:**

```rust
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message, CloseFrame};
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;

/// WebSocket 连接状态注册表
pub struct ConnectionRegistry {
    connections: HashMap<Uuid, mpsc::Sender<Message>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub async fn register(&mut self, id: Uuid, tx: mpsc::Sender<Message>) {
        self.connections.insert(id, tx);
    }

    pub async fn unregister(&mut self, id: Uuid) {
        self.connections.remove(&id);
    }

    pub async fn send_to(&self, id: Uuid, msg: Message) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = self.connections.get(&id) {
            tx.send(msg).await?;
        }
        Ok(())
    }

    pub async fn broadcast(&self, msg: Message) {
        for tx in self.connections.values() {
            tx.send(msg.clone()).await.ok();
        }
    }
}

/// WebSocket 处理器
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    params: QueryParams,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, params))
}

async fn handle_socket(socket: WebSocket, params: QueryParams) {
    let id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel(32);

    // 注册连接
    REGISTRY.lock().await.register(id, tx).await;

    let (mut sender, mut receiver) = socket.split();

    // 发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // 接收任务
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // 执行 Python handler
                    execute_ws_function(&text, &params).await;
                }
                Message::Close(frame) => {
                    debug!("WebSocket closed: {:?}", frame);
                    break;
                }
                _ => {}
            }
        }
    });

    // 等待任一任务完成
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // 清理连接
    REGISTRY.lock().await.unregister(id).await;
}
```

#### 5.3.3 WebSocket Registry (`src/websockets/registry.rs`)

**当前实现 (Actix Actor):**
```rust
pub struct WebSocketRegistry {
    connections: HashMap<Uuid, Addr<WebSocketConnector>>,
}

impl Actor for WebSocketRegistry {
    type Context = Context<Self>;
}

impl Handler<Connect> for WebSocketRegistry {
    type Result = ();

    fn handle(&mut self, msg: Connect, _ctx: &mut Self::Context) {
        // ...
    }
}
```

**目标实现 (Axum + Arc<Mutex>):**
```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

pub type SharedRegistry = Arc<Mutex<ConnectionRegistry>>;

pub struct ConnectionRegistry {
    connections: HashMap<Uuid, mpsc::Sender<Message>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub async fn connect(&mut self, id: Uuid, tx: mpsc::Sender<Message>) {
        self.connections.insert(id, tx);
    }

    pub async fn disconnect(&mut self, id: Uuid) {
        self.connections.remove(&id);
    }

    pub async fn send_message(&self, id: Uuid, msg: Message) -> Result<(), SendError> {
        if let Some(tx) = self.connections.get(&id) {
            tx.send(msg).await.map_err(|_| SendError)?;
        }
        Ok(())
    }

    pub async fn broadcast(&self, msg: Message) {
        futures::future::join_all(
            self.connections.values().iter().map(|tx| tx.send(msg.clone()))
        ).await;
    }
}
```

---

## 6. 实施计划

### 6.1 阶段划分

| 阶段 | 内容 | 预计时间 | 风险 |
|------|------|----------|------|
| **阶段 1** | 基础设施准备 | 1-2 周 | 低 |
| **阶段 2** | 核心类型迁移 | 2-3 周 | 中 |
| **阶段 3** | 路由系统迁移 | 1-2 周 | 低 |
| **阶段 4** | WebSocket 迁移 | 2-3 周 | 高 |
| **阶段 5** | Server 整合 | 1-2 周 | 中 |
| **阶段 6** | 测试验证 | 1-2 周 | 中 |

**总计**: 8-12 周（单人全职）

### 6.2 详细任务分解

#### 阶段 1: 基础设施准备 (Week 1-2)

- [ ] 更新 Cargo.toml，添加 Axum 依赖
- [ ] 配置 feature flags (`actix-runtime`, `axum-runtime`)
- [ ] 创建 `src/axum_adapter/` 目录结构
- [ ] 建立并行测试环境
- [ ] 验证基础构建

**交付物:**
- 可并行构建的配置
- 基础测试框架

#### 阶段 2: 核心类型迁移 (Week 3-5)

- [ ] 实现 `From<HttpRequest>` for Request (Axum 版本)
- [ ] 实现 `IntoResponse` for Response
- [ ] 实现 Headers 转换
- [ ] 适配 executors 模块
- [ ] 编写类型转换单元测试

**交付物:**
- `src/types/request.rs` - `from_axum_request()`
- `src/types/response.rs` - `impl IntoResponse`
- `src/types/headers.rs` - `from_http_headers()`

#### 阶段 3: 路由系统迁移 (Week 6-7)

- [ ] 更新 routers/ 模块类型签名
- [ ] 实现 Axum Router 构建函数
- [ ] 集成 matchit 路由
- [ ] 静态文件服务迁移 (tower-http)
- [ ] Multipart 处理迁移

**交付物:**
- `src/routers/` - 类型适配完成
- `fn make_router()` - Axum Router 构建

#### 阶段 4: WebSocket 迁移 (Week 8-10)

- [ ] 设计 ConnectionRegistry (Arc<Mutex> 模式)
- [ ] 重写 `src/websockets/mod.rs`
- [ ] 重写 `src/websockets/registry.rs`
- [ ] 实现消息广播机制
- [ ] WebSocket 单元测试

**交付物:**
- 完整的 WebSocket Stream 实现
- 连接注册表
- 广播机制

#### 阶段 5: Server 整合 (Week 11-12)

- [ ] 重写 `src/server.rs` 的 `start()` 方法
- [ ] 实现 Tokio runtime 初始化
- [ ] 集成所有模块
- [ ] 共享 Socket 适配
- [ ] 端到端测试

**交付物:**
- 可运行的 Axum Server
- 完整的集成测试

#### 阶段 6: 测试验证 (Week 13-14)

- [ ] 运行所有现有测试
- [ ] 性能基准对比
- [ ] 内存/CPU 分析
- [ ] 文档更新
- [ ] 发布说明准备

**交付物:**
- 测试报告
- 性能对比报告
- 迁移指南

### 6.3 优先级矩阵

| 优先级 | 模块 | 依赖 | 风险 |
|--------|------|------|------|
| P0 | `src/types/response.rs` | 无 | 低 |
| P0 | `src/types/request.rs` | 无 | 低 |
| P0 | `src/server.rs` | Response, Request | 中 |
| P1 | `src/executors/mod.rs` | Request, Response | 低 |
| P1 | `src/routers/*` | executors | 低 |
| P2 | `src/websockets/mod.rs` | 无 (独立) | 高 |
| P2 | `src/websockets/registry.rs` | websockets/mod | 高 |

---

## 7. 测试策略

### 7.1 单元测试

**目标**: 覆盖所有类型转换和核心逻辑

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_request_from_axum_request() {
        // 测试 Request 从 Axum 请求的转换
    }

    #[test]
    fn test_response_into_response() {
        // 测试 Response 转换为 Axum 响应
    }
}
```

### 7.2 集成测试

**目标**: 验证端到端功能

```rust
#[tokio::test]
async fn test_http_get_request() {
    let app = make_test_app();
    let client = TestClient::new(app);

    let resp = client.get("/").send().await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_websocket_connection() {
    let app = make_test_app();
    let client = TestClient::new(app);

    let ws = client.websocket("/ws").await;
    // ... WebSocket 测试
}
```

### 7.3 性能基准

**工具**: Criterion.rs

```rust
fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("request handling", |b| {
        b.iter(|| handle_request(black_box(&request)))
    });
}
```

### 7.4 并行测试

在迁移期间，保持 Actix 和 Axum 两套测试：

```bash
# Actix 测试
cargo test --features actix-runtime

# Axum 测试
cargo test --features axum-runtime --no-default-features
```

---

## 8. 风险缓解

### 8.1 已识别风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| WebSocket 功能缺失 | 高 | 中 | 优先实现核心功能，渐进式完善 |
| 性能回归 | 高 | 低 | 持续基准测试，保留 Actix fallback |
| Python 层不兼容 | 高 | 低 | 保持 PyO3 FFI 边界不变 |
| 测试覆盖不足 | 中 | 中 | 优先保证核心路径覆盖 |
| 迁移时间超期 | 中 | 中 | 分阶段交付，优先 P0 任务 |

### 8.2 回滚计划

1. **保留 Actix 依赖**: Cargo.toml 中保留 Actix 依赖
2. **Feature Flag 切换**: 通过 feature flag 切换运行时
3. **并行测试**: 确保 Actix 测试始终通过
4. **版本控制**: 每阶段提交，易于回滚

---

## 9. 成功标准

### 9.1 功能标准

- [ ] 所有 HTTP 方法支持 (GET, POST, PUT, DELETE, etc.)
- [ ] 中间件系统正常工作
- [ ] WebSocket 完全支持 (connect, message, close)
- [ ] 静态文件服务正常
- [ ] Multipart 表单支持
- [ ] 多进程/多 Worker 支持

### 9.2 性能标准

- [ ] 吞吐量不低于 Actix 的 90%
- [ ] P99 延迟不增加超过 20%
- [ ] 内存使用不增加超过 10%
- [ ] 编译时间减少 20%+

### 9.3 质量标准

- [ ] 单元测试覆盖率 > 80%
- [ ] 所有集成测试通过
- [ ] 无内存泄漏
- [ ] 文档完整

---

## 10. 参考资源

### 10.1 官方文档

- [Axum Documentation](https://docs.rs/axum/)
- [Tower Documentation](https://docs.rs/tower/)
- [Tokio Documentation](https://tokio.rs/)
- [pyo3-asyncio Documentation](https://docs.rs/pyo3-asyncio/)

### 10.2 示例代码

- [Axum Examples](https://github.com/tokio-rs/axum/tree/main/examples)
- [Tower HTTP Examples](https://github.com/tower-rs/tower-http/tree/master/examples)

### 10.3 迁移参考

- [Actix to Axum Migration Guide (社区)](https://github.com/tokio-rs/axum/discussions/)
- [Tower Middleware Documentation](https://docs.rs/tower-http/)

---

## 11. 附录

### 11.1 关键代码位置

| 功能 | 当前文件 (Actix) | 目标文件 (Axum) |
|------|-----------------|-----------------|
| HTTP Server | `src/server.rs` | `src/server.rs` |
| WebSocket | `src/websockets/mod.rs` | `src/websockets/mod.rs` |
| Registry | `src/websockets/registry.rs` | `src/websockets/registry.rs` |
| Request | `src/types/request.rs` | `src/types/request.rs` |
| Response | `src/types/response.rs` | `src/types/response.rs` |
| Headers | `src/types/headers.rs` | `src/types/headers.rs` |
| Executors | `src/executors/mod.rs` | `src/executors/mod.rs` |
| Routers | `src/routers/` | `src/routers/` |
| Socket | `src/shared_socket.rs` | `src/shared_socket.rs` |

### 11.2 代码行数估算

| 模块 | 当前 (行) | 预计 (行) | 变化 |
|------|----------|----------|------|
| server.rs | ~560 | ~350 | -38% |
| websockets/ | ~400 | ~500 | +25% |
| types/ | ~600 | ~550 | -8% |
| routers/ | ~800 | ~750 | -6% |
| executors/ | ~300 | ~280 | -7% |
| **总计** | **~2660** | **~2430** | **-9%** |

### 11.3 联系信息

如有问题，请参考：
- 项目 GitHub Issues
- Axon Discord 社区
- Axum GitHub Discussions
