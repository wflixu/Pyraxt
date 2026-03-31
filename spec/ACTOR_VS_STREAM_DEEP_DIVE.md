# Actor 模型 vs Stream 模型：深度解析

## 目录

1. [Actor 模型运行机理](#1-actor-模型运行机理)
2. [Stream 模型运行机理](#2-stream-模型运行机理)
3. [核心差异对比](#3-核心差异对比)
4. [优劣势深度分析](#4-优劣势深度分析)
5. [实际代码对比](#5-实际代码对比)
6. [选择指南](#6-选择指南)

---

## 1. Actor 模型运行机理

### 1.1 核心概念

Actor 模型是由 **Carl Hewitt** 在 1973 年提出的并发计算模型。它的核心思想是：

> **Actor 是并发计算的基本单元，每个 Actor 都有：**
> - **独立的状态**
> - **一个 Mailbox（邮箱）**
> - **一个行为（处理逻辑）**

```
┌─────────────────────────────────────────────────────────────┐
│                         Actor                                │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    状态 (State)                        │  │
│  │  { name: "Server", connections: 42, ... }             │  │
│  └───────────────────────────────────────────────────────┘  │
│                            │                                 │
│                            ▼                                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    Mailbox                             │  │
│  │  ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐                  │  │
│  │  │Msg1 │→ │Msg2 │→ │Msg3 │→ │Msg4 │ → ...           │  │
│  │  └─────┘  └─────┘  └─────┘  └─────┘                  │  │
│  └───────────────────────────────────────────────────────┘  │
│                            │                                 │
│                            ▼                                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    行为 (Behavior)                     │  │
│  │  fn handle(&mut self, msg: Msg, ctx: &mut Context)    │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 运行机理详解

#### 1.2.1 Actor 生命周期

```
         ┌──────────────┐
         │    创建       │
         │  (Created)    │
         └──────┬───────┘
                │
                ▼
         ┌──────────────┐
         │    启动       │
         │  (Started)    │ ← started() 钩子调用
         └──────┬───────┘
                │
                ▼
    ┌───────────────────────┐
    │      运行中            │
    │   (Running/Processing) │ ← 循环处理 Mailbox 消息
    └───────┬───────────────┘
            │
    ┌───────┴───────┐
    │               │
    ▼               ▼
┌──────────┐   ┌──────────┐
│  停止     │   │  重启     │
│ (Stopped)│   │(Restart) │
└──────────┘   └──────────┘
```

#### 1.2.2 消息处理循环

```rust
// 伪代码：Actor 内部运行循环
impl<A: Actor> ActorRunner<A> {
    async fn run(&mut self) {
        // 1. 调用 started() 钩子
        self.actor.started(&mut self.context);

        // 2. 消息处理循环
        loop {
            // 从 Mailbox 接收消息
            let msg = self.mailbox.recv().await;

            match msg {
                Some(Message::Regular(m)) => {
                    // 处理普通消息
                    self.actor.handle(m, &mut self.context);
                }
                Some(Message::Stop) => {
                    // 收到停止信号
                    break;
                }
                None => {
                    // Mailbox 关闭，退出循环
                    break;
                }
            }
        }

        // 3. 调用 stopped() 钩子
        self.actor.stopped(&mut self.context);
    }
}
```

#### 1.2.3 Actor 通信图

```
┌─────────────┐                          ┌─────────────┐
│   Actor A   │                          │   Actor B   │
│             │                          │             │
│  ┌───────┐  │                          │  ┌───────┐  │
│  │Addr B │  │─────────────────────────▶│  │Mailbox│  │
│  └───────┘  │         send(Msg)        │  └───────┘  │
│             │                          │      │      │
│             │                          │      ▼      │
│             │                          │  ┌───────┐  │
│             │                          │  │Handler│  │
│             │                          │  └───────┘  │
└─────────────┘                          └─────────────┘
```

### 1.3 Actix Actor 实现示例

```rust
use actix::{Actor, ActorContext, Handler, Message, Addr};

// 1. 定义消息
#[derive(Message)]
#[rtype(result = "()")]
struct Connect {
    id: Uuid,
}

// 2. 定义 Actor
struct WebSocketActor {
    id: Uuid,
    registry: Addr<RegistryActor>,
    connections: HashMap<Uuid, Addr<WebSocketActor>>,
}

// 3. 实现 Actor trait
impl Actor for WebSocketActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Actor {} started", self.id);
        // 注册自己
        self.registry.do_send(Register { id: self.id, addr: ctx.address() });
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        println!("Actor {} stopped", self.id);
        // 注销自己
        self.registry.do_send(Unregister { id: self.id });
    }
}

// 4. 实现消息处理器
impl Handler<Connect> for WebSocketActor {
    type Result = ();

    fn handle(&mut self, msg: Connect, ctx: &mut Self::Context) {
        // 处理消息
        println!("Received Connect from {}", msg.id);
    }
}

// 5. 启动 Actor
let addr = WebSocketActor {
    id: Uuid::new_v4(),
    registry: registry_addr,
    connections: HashMap::new(),
}.start();

// 6. 发送消息
addr.send(Connect { id: Uuid::new_v4() }).await;
```

### 1.4 Actor 模型关键特性

| 特性 | 说明 | 实现方式 |
|------|------|----------|
| **封装性** | 状态对外不可见 | 只能通过消息访问 |
| **隔离性** | Actor 之间不共享状态 | 消息传递 |
| **并发性** | 多个 Actor 并行执行 | 每个 Actor 独立调度 |
| **位置透明** | 不知道 Actor 在哪里 | 通过 Addr 寻址 |
| **生命周期** | 有明确的启动/停止 | started()/stopped() 钩子 |

---

## 2. Stream 模型运行机理

### 2.1 核心概念

Stream 模型基于 **响应式编程** 和 **函数式编程** 思想。核心思想是：

> **数据像水流一样流动，通过管道（channel）在组件之间传递**

```
数据源 ──────▶ [Channel] ──────▶ 处理器 ──────▶ 输出
(Stream)                        (Handler)
```

### 2.2 运行机理详解

#### 2.2.1 Stream Trait

```rust
// Rust Stream trait 定义
pub trait Stream {
    type Item;

    // 获取下一个元素
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Option<Self::Item>>;
}
```

#### 2.2.2 Channel 架构

```
┌─────────────────────────────────────────────────────────────┐
│                    mpsc::Channel                            │
│  (Multi-Producer, Single-Consumer)                          │
│                                                             │
│  ┌──────────┐                                              │
│  │ Producer │────┐                                         │
│  └──────────┘    │                                         │
│                  │    ┌─────────────┐                      │
│  ┌──────────┐    ├───▶│   Buffer    │                      │
│  │ Producer │────┤    │  [队列]     │                      │
│  └──────────┘    │    └──────┬──────┘                      │
│                  │           │                              │
│  ┌──────────┐    │           ▼                              │
│  │ Producer │────┤    ┌─────────────┐                      │
│  └──────────┘    │    │  Consumer   │                      │
│                  │    │  (Receiver) │                      │
│                  │    └─────────────┘                      │
└─────────────────────────────────────────────────────────────┘
```

#### 2.2.3 消息处理循环

```rust
// Stream 模型的消息处理
async fn handle_socket(mut socket: WebSocket) {
    // 1. 分割双向流
    let (mut tx, mut rx) = socket.split();

    // 2. 创建内部 channel
    let (msg_tx, mut msg_rx) = mpsc::channel(32);

    // 3. 发送任务
    let send_task = tokio::spawn(async move {
        // 从 channel 接收并发送
        while let Some(msg) = msg_rx.recv().await {
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // 4. 接收任务
    let recv_task = tokio::spawn(async move {
        // 从 socket 接收并处理
        while let Some(Ok(msg)) = socket.next().await {
            match msg {
                Message::Text(text) => {
                    // 处理消息
                    handle_message(&text).await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // 5. 等待任一任务完成
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

### 2.3 Axum Stream 实现示例

```rust
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade, Message},
    response::IntoResponse,
};
use tokio::sync::mpsc;
use futures::{sink::SinkExt, stream::StreamExt};

// 1. 定义 WebSocket handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

// 2. 处理 WebSocket 连接
async fn handle_socket(socket: WebSocket) {
    // 分割为发送和接收两部分
    let (mut tx, mut rx) = socket.split();

    // 创建消息 channel
    let (msg_tx, mut msg_rx) = mpsc::channel(32);

    // 发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if tx.send(msg).await.is_err() {
                println!("Failed to send message");
                break;
            }
        }
    });

    // 接收任务
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                Message::Text(text) => {
                    println!("Received: {}", text);
                    // 处理消息
                    process_message(&text).await;
                }
                Message::Close(close_frame) => {
                    println!("Connection closed: {:?}", close_frame);
                    break;
                }
                Message::Ping(bytes) => {
                    println!("Ping received");
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
}

async fn process_message(text: &str) {
    // 实际的消息处理逻辑
    println!("Processing: {}", text);
}
```

### 2.4 Stream 模型关键特性

| 特性 | 说明 | 实现方式 |
|------|------|----------|
| **数据流** | 数据像流一样传递 | `Stream` trait |
| **组合性** | 可以组合多个 Stream | `map`, `filter`, `merge` |
| **背压** | 下游控制上游速率 | 有界 channel |
| **显式生命周期** | 手动管理 task | `tokio::spawn` |
| **状态共享** | 通过 `Arc<Mutex<>>` | 显式同步 |

---

## 3. 核心差异对比

### 3.1 架构对比图

```
┌─────────────────────────────────┐  ┌─────────────────────────────────┐
│         Actor 模型               │  │         Stream 模型              │
│                                 │  │                                 │
│  ┌───────────────────────────┐  │  │  ┌───────────────────────────┐  │
│  │      WebSocketActor       │  │  │  │     handle_socket()       │  │
│  │                           │  │  │  │                           │  │
│  │  ┌─────────────────────┐  │  │  │  │  ┌─────────────────────┐  │  │
│  │  │      Mailbox        │  │  │  │  │  │   mpsc::Channel     │  │  │
│  │  │  [Msg1, Msg2, ...]  │  │  │  │  │  │   [Msg1, Msg2, ...] │  │  │
│  │  └─────────────────────┘  │  │  │  │  └─────────────────────┘  │  │
│  │            │              │  │  │  │            │              │  │
│  │            ▼              │  │  │  │            ▼              │  │
│  │  ┌─────────────────────┐  │  │  │  │  ┌─────────────────────┐  │  │
│  │  │  handle() 方法       │  │  │  │  │  │  async 循环          │  │  │
│  │  │  (自动调用)          │  │  │  │  │  │  (手动编写)          │  │  │
│  │  └─────────────────────┘  │  │  │  │  └─────────────────────┘  │  │
│  │                           │  │  │  │                           │  │
│  │  • 自动生命周期管理       │  │  │  │  • 手动生命周期管理        │  │
│  │  • 内置 Mailbox           │  │  │  │  • 手动创建 Channel        │  │
│  │  • 状态封装在 Actor 内     │  │  │  │  • 状态显式共享 (Arc)      │  │
│  └───────────────────────────┘  │  │  └───────────────────────────┘  │
└─────────────────────────────────┘  └─────────────────────────────────┘
```

### 3.2 消息传递对比

| 方面 | Actor 模型 | Stream 模型 |
|------|-----------|-------------|
| **发送方式** | `addr.send(msg).await` | `tx.send(msg).await` |
| **接收方式** | 自动 (Mailbox → handle) | 手动 (`rx.recv().await`) |
| **消息队列** | Mailbox (内置) | Channel (手动创建) |
| **排序保证** | FIFO | FIFO (取决于实现) |
| **背压** | 自动 (容量限制) | 手动 (有界 channel) |

### 3.3 状态管理对比

| 方面 | Actor 模型 | Stream 模型 |
|------|-----------|-------------|
| **状态位置** | Actor 内部字段 | `Arc<Mutex<State>>` |
| **访问方式** | 通过消息 | 直接锁访问 |
| **同步机制** | 消息序列化 | Mutex/RwLock |
| **封装性** | 好 | 差 |

**Actor 状态管理**:
```rust
struct Actor {
    state: State,  // 私有
}

// 只能通过消息访问
impl Handler<GetState> for Actor {
    type Result = State;
    fn handle(&mut self, _msg: GetState, _ctx: &mut Context) -> Self::Result {
        self.state.clone()  // 通过方法访问
    }
}
```

**Stream 状态管理**:
```rust
let state = Arc::new(Mutex::new(State::new()));

// 直接访问
{
    let mut s = state.lock().await;
    s.modify();  // 直接修改
}
```

### 3.4 生命周期对比

| 阶段 | Actor 模型 | Stream 模型 |
|------|-----------|-------------|
| **创建** | `MyActor.start()` | `tokio::spawn(async {} )` |
| **启动** | `started()` 自动调用 | 手动初始化 |
| **运行** | 自动循环处理消息 | 手动编写循环 |
| **停止** | `stopped()` 自动调用 | 手动清理 |

### 3.5 错误处理对比

| 方面 | Actor 模型 | Stream 模型 |
|------|-----------|-------------|
| **错误传播** | 返回给发送者 | 手动传播 |
| **重启机制** | 内置监督 | 手动实现 |
| **错误隔离** | Actor 边界 | 需要手动处理 |

**Actor 错误处理**:
```rust
// Actix 监督机制
impl SupervisorProtocol for MyActor {
    fn supervising(&mut self, child: &Addr<Self>) {
        // 子 Actor 失败时自动重启
    }
}
```

**Stream 错误处理**:
```rust
// 手动错误处理
match result {
    Ok(value) => { /* 处理 */ }
    Err(e) => {
        // 需要手动决定：重试/忽略/传播
        log_error(&e);
    }
}
```

---

## 4. 优劣势深度分析

### 4.1 Actor 模型优势

#### ✅ 优势 1: 优秀的封装性

```rust
// Actor: 状态完全封装
struct ServerActor {
    connections: HashMap<Uuid, Connection>,
    config: ServerConfig,
    stats: Stats,
}

// 外部只能发送消息，无法直接访问状态
addr.send(GetStats).await?;  // ✓ 通过消息访问
// addr.stats  // ✗ 无法访问
```

**好处**:
- 状态不会被意外修改
- 线程安全由模型保证
- 易于推理和维护

#### ✅ 优势 2: 自动生命周期管理

```rust
impl Actor for WebSocketActor {
    fn started(&mut self, ctx: &mut Context) {
        // 自动调用：注册、初始化
        println!("Actor started");
    }

    fn stopped(&mut self, ctx: &mut Context) {
        // 自动调用：清理、注销
        println!("Actor stopped");
    }
}
```

**好处**:
- 不会忘记清理资源
- 统一的初始化和销毁模式
- 易于调试（有明确的起止点）

#### ✅ 优势 3: 内置背压

```rust
// 有界 Mailbox
let addr = MyActor.start_with_context(|ctx| {
    ctx.set_mailbox_capacity(10);  // 容量限制
});

// 满时自动阻塞
addr.send(msg).await;  // 如果 Mailbox 满，这里会等待
```

**好处**:
- 防止内存溢出
- 自动流量控制
- 系统更稳定

#### ✅ 优势 4: 监督层次

```rust
// 父 Actor 可以监控子 Actor
struct SupervisorActor {
    children: Vec<Addr<WorkerActor>>,
}

impl SupervisorProtocol for SupervisorActor {
    fn child_stopped(&mut self, child: &Addr<WorkerActor>) {
        // 自动重启失败的子 Actor
        let new_child = WorkerActor.start();
        self.children.push(new_child);
    }
}
```

**好处**:
- 错误隔离
- 自动恢复
- 系统更健壮

### 4.2 Actor 模型劣势

#### ❌ 劣势 1: 学习曲线陡峭

```rust
// 需要理解的概念
- Actor
- Context
- Addr
- Mailbox
- Handler<Message>
- StreamHandler
- SupervisorProtocol
- ActorFuture
```

**影响**:
- 新人上手慢
- 需要阅读大量文档
- 调试困难

#### ❌ 劣势 2: 灵活性受限

```rust
// Actor 必须遵循固定模式
impl Actor for MyActor { /* ... */ }
impl Handler<Msg1> for MyActor { /* ... */ }
impl Handler<Msg2> for MyActor { /* ... */ }

// 难以与其他模式组合
// 例如：难以直接使用 Tower 中间件
```

**影响**:
- 代码必须遵循 Actor 模式
- 难以复用现有库
- 生态封闭

#### ❌ 劣势 3: 性能开销

```
┌────────────────────────────────────────────┐
│  消息 → Mailbox 入队 → 调度 → handle()      │
│         ↑        ↑         ↑        ↑      │
│         │        │         │        │      │
│      复制    锁竞争    上下文    虚调用     │
│                       切换                │
└────────────────────────────────────────────┘
```

**开销来源**:
- 消息复制
- Mailbox 锁竞争
- Actor 调度
- 虚函数调用

#### ❌ 劣势 4: 调试困难

```rust
// 问题：消息为什么没处理？

// 可能的原因：
// 1. 消息没发送到 Mailbox
// 2. Mailbox 满了
// 3. Actor 已经停止
// 4. 消息被丢弃了
// 5. handle() 中有 panic

// 调试困难：异步、多 Actor、消息队列
```

### 4.3 Stream 模型优势

#### ✅ 优势 1: 符合 Rust 标准

```rust
// 使用标准 trait
impl Stream for MyStream {
    type Item = T;
    fn poll_next(...) { /* ... */ }
}

// 可以直接使用标准库和 tokio
use tokio::sync::mpsc;
use futures::stream::StreamExt;
```

**好处**:
- 学习成本低
- 文档丰富
- 生态兼容性好

#### ✅ 优势 2: 灵活性高

```rust
// 可以自由组合
let stream = source
    .filter(|x| x > 0)
    .map(|x| x * 2)
    .buffer_unordered(10)
    .forward(sink);

// 可以使用 Tower 中间件
let app = Router::new()
    .layer(TraceLayer::new_for_http())
    .layer(TimeoutLayer::new(Duration::from_secs(30)));
```

**好处**:
- 代码更简洁
- 可以复用现有库
- 易于定制

#### ✅ 优势 3: 性能更好

```
┌────────────────────────────────────────────┐
│  Channel 发送 → Receiver 接收 → 处理        │
│         ↑              ↑           ↑       │
│         │              │           │       │
│      无锁 (部分)     零拷贝     直接调用   │
└────────────────────────────────────────────┘
```

**性能优势**:
- 更少的复制
- 更少的锁
- 更直接的调用

#### ✅ 优势 4: 调试容易

```rust
// 代码流程清晰
async fn handle() {
    // 1. 接收
    let msg = rx.recv().await;

    // 2. 处理
    let result = process(msg).await;

    // 3. 发送
    tx.send(result).await;

    // 可以直接设置断点、打印日志
}
```

**好处**:
- 流程直观
- 容易设置断点
- 日志清晰

### 4.4 Stream 模型劣势

#### ❌ 劣势 1: 样板代码多

```rust
// 需要手动管理
let (tx, rx) = mpsc::channel(32);  // 创建 channel
let task = tokio::spawn(async move {  // 创建 task
    while let Some(msg) = rx.recv().await {  // 手动循环
        // 处理
    }
});  // 手动等待
```

**对比 Actor**:
```rust
// Actor 自动处理
impl Handler<Msg> for MyActor {
    fn handle(&mut self, msg: Msg, ctx: &mut Context) {
        // 直接处理
    }
}
```

#### ❌ 劣势 2: 状态管理复杂

```rust
// 需要显式共享状态
let state = Arc::new(Mutex::new(State::new()));

// 访问时需要锁
{
    let mut s = state.lock().await;
    s.modify();
}

// 容易忘记释放锁，导致死锁
```

#### ❌ 劣势 3: 无内置监督

```rust
// Task 失败后不会自动重启
let task = tokio::spawn(async {
    // 如果 panic，task 就消失了
});

// 需要手动实现重启逻辑
loop {
    let task = tokio::spawn(worker());
    if task.await.is_err() {
        // 重启
        continue;
    }
}
```

#### ❌ 劣势 4: 生命周期管理繁琐

```rust
// 需要手动管理
let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

let task = tokio::spawn(async move {
    tokio::select! {
        _ = do_work() => {},
        _ = &mut shutdown_rx => {
            // 手动清理
            cleanup().await;
        }
    }
});

// 停止时需要发送信号
shutdown_tx.send(()).ok();
```

---

## 5. 实际代码对比

### 5.1 WebSocket 连接管理

#### Actor 实现 (Actix)

```rust
use actix::{Actor, StreamHandler, Addr};
use actix_web_actors::ws;

pub struct WsActor {
    id: Uuid,
    registry: Addr<RegistryActor>,
}

impl Actor for WsActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        // 自动调用：注册
        self.registry.do_send(Register { id: self.id });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        // 自动调用：注销
        self.registry.do_send(Unregister { id: self.id });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsActor {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Text(text) => {
                println!("Received: {}", text);
            }
            ws::Message::Close(frame) => {
                ctx.close(frame);
                ctx.stop();  // 停止 Actor
            }
            _ => {}
        }
    }
}
```

#### Stream 实现 (Axum)

```rust
use axum::extract::ws::{WebSocket, Message};
use tokio::sync::mpsc;
use futures::{sink::SinkExt, stream::StreamExt};

pub async fn handle_socket(
    socket: WebSocket,
    registry: Arc<Mutex<Registry>>,
) {
    let id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel(32);

    // 手动注册
    {
        let mut r = registry.lock().await;
        r.connections.insert(id, tx);
    }

    let (mut tx_socket, mut rx_socket) = socket.split();

    // 发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if tx_socket.send(msg).await.is_err() {
                break;
            }
        }
    });

    // 接收任务
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = rx_socket.next().await {
            match msg {
                Message::Text(text) => {
                    println!("Received: {}", text);
                }
                Message::Close(frame) => {
                    break;
                }
                _ => {}
            }
        }
    });

    // 等待
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // 手动注销
    {
        let mut r = registry.lock().await;
        r.connections.remove(&id);
    }
}
```

#### 代码对比

| 方面 | Actor | Stream |
|------|-------|--------|
| **代码行数** | ~35 行 | ~55 行 |
| **生命周期** | 自动 (started/stopped) | 手动 (注册/注销) |
| **消息处理** | 自动循环 (handle) | 手动循环 |
| **任务管理** | 自动 | 手动 `tokio::spawn` |
| **清理** | 自动 | 手动 |

### 5.2 广播功能

#### Actor 实现

```rust
// RegistryActor
struct RegistryActor {
    connections: HashMap<Uuid, Addr<WsActor>>,
}

impl Handler<Broadcast> for RegistryActor {
    type Result = ();

    fn handle(&mut self, msg: Broadcast, _ctx: &mut Context) {
        // 广播给所有连接
        for (id, addr) in &self.connections {
            addr.do_send(SendMessage {
                id: *id,
                msg: msg.text.clone(),
            });
        }
    }
}
```

#### Stream 实现

```rust
struct Registry {
    connections: HashMap<Uuid, mpsc::Sender<Message>>,
}

impl Registry {
    async fn broadcast(&self, text: String) {
        let msg = Message::Text(text.clone());

        for (id, tx) in &self.connections {
            tx.send(msg.clone()).await.ok();
        }
    }
}
```

### 5.3 状态管理

#### Actor 实现

```rust
struct StatsActor {
    request_count: u64,
    error_count: u64,
}

impl Handler<IncrementRequest> for StatsActor {
    type Result = ();

    fn handle(&mut self, _msg: IncrementRequest, _ctx: &mut Context) {
        self.request_count += 1;  // 直接修改内部状态
    }
}

impl Handler<GetStats> for StatsActor {
    type Result = Stats;

    fn handle(&mut self, _msg: GetStats, _ctx: &mut Context) -> Self::Result {
        Stats {
            requests: self.request_count,
            errors: self.error_count,
        }
    }
}
```

#### Stream 实现

```rust
struct Stats {
    request_count: AtomicU64,
    error_count: AtomicU64,
}

// 使用原子类型避免锁
async fn increment_request(stats: Arc<Stats>) {
    stats.request_count.fetch_add(1, Ordering::SeqCst);
}

async fn get_stats(stats: Arc<Stats>) -> Stats {
    Stats {
        requests: stats.request_count.load(Ordering::SeqCst),
        errors: stats.error_count.load(Ordering::SeqCst),
    }
}
```

---

## 6. 选择指南

### 6.1 何时选择 Actor 模型

✅ **适合 Actor 的场景**:

| 场景 | 说明 |
|------|------|
| 复杂状态机 | 多个状态，状态之间有复杂转换逻辑 |
| 需要监督 | 子任务失败需要自动重启 |
| 领域建模 | 天然有"角色"概念（如玩家、房间） |
| 分布式系统 | 需要跨节点通信（如 Akka Cluster） |
| 热升级 | 运行时替换逻辑 |

**典型应用**:
- 游戏服务器（玩家、房间）
- 聊天系统（用户、群组）
- IoT 设备管理

### 6.2 何时选择 Stream 模型

✅ **适合 Stream 的场景**:

| 场景 | 说明 |
|------|------|
| Web 服务 | HTTP 请求处理 |
| WebSocket | 连接管理、消息收发 |
| 数据管道 | ETL、流式处理 |
| API 网关 | 请求路由、转换 |
| 简单 CRUD | 数据库操作 |

**典型应用**:
- Web 框架（Axum, Hyper）
- API 服务
- 数据流处理

### 6.3 Axon 的选择

| Axon 需求 | 适合模型 | 理由 |
|-----------|---------|------|
| HTTP 服务器 | Stream | 标准 Web 模式 |
| WebSocket | Stream | 简单连接管理 |
| 中间件 | Stream | Tower 生态 |
| 静态文件 | Stream | 简单文件服务 |
| 多进程 | 两者皆可 | 与模型无关 |

**结论**: Axon 适合 **Stream 模型 (Axum)**

---

## 7. 总结

### 核心对比表

| 维度 | Actor 模型 | Stream 模型 |
|------|-----------|-------------|
| **学习曲线** | 陡峭 | 平缓 |
| **代码量** | 少 | 多 |
| **封装性** | 优秀 | 一般 |
| **灵活性** | 受限 | 高 |
| **性能** | 有开销 | 更优 |
| **调试** | 困难 | 容易 |
| **生态** | 封闭 | 开放 |
| **监督** | 内置 | 手动 |
| **生命周期** | 自动 | 手动 |

### 最终建议

```
┌─────────────────────────────────────────────────────────┐
│                                                         │
│  选择 Actor 模型，如果：                                 │
│  • 需要复杂状态管理                                     │
│  • 需要监督和自重启                                     │
│  • 领域模型天然有"角色"概念                              │
│                                                         │
│  选择 Stream 模型，如果：                                │
│  • 构建 Web 服务/API                                     │
│  • 需要与现有 Rust 生态集成                             │
│  • 追求性能和简洁                                       │
│  • 团队更熟悉标准 Rust 模式                             │
│                                                         │
│  对于 Axon: ✅ 推荐 Stream 模型 (Axum)                   │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

**参考资源**:
- [Actor Model (Wikipedia)](https://en.wikipedia.org/wiki/Actor_model)
- [Actix Documentation](https://actix.rs/docs/)
- [Tokio Stream Documentation](https://docs.rs/tokio-stream/latest/tokio_stream/)
- [Axum Examples](https://github.com/tokio-rs/axum/tree/main/examples)
