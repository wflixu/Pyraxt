use pyo3::prelude::*;
use pyo3_asyncio::TaskLocals;

use crate::types::function_info::FunctionInfo;
use crate::websockets::WebSocketConnector;

fn get_function_output<'a>(
    function: &'a FunctionInfo,
    fn_msg: Option<String>,
    py: Python<'a>,
    ws: &WebSocketConnector,
) -> Result<&'a PyAny, PyErr> {
    let handler = function.handler.as_ref(py);

    // this makes the request object accessible across every route

    let args = function.args.as_ref(py);
    let kwargs = function.kwargs.as_ref(py);

    match function.number_of_params {
        0 => handler.call0(),
        1 => {
            if args.get_item("ws")?.is_some() {
                handler.call1((ws.clone(),))
            } else if args.get_item("msg")?.is_some() {
                handler.call1((fn_msg.unwrap_or_default(),))
            } else {
                handler.call((), Some(kwargs))
            }
        }
        2 => {
            if args.get_item("ws")?.is_some() && args.get_item("msg")?.is_some() {
                handler.call1((ws.clone(), fn_msg.unwrap_or_default()))
            } else if args.get_item("ws")?.is_some() {
                handler.call((ws.clone(),), Some(kwargs))
            } else if args.get_item("msg")?.is_some() {
                handler.call((fn_msg.unwrap_or_default(),), Some(kwargs))
            } else {
                handler.call((), Some(kwargs))
            }
        }
        3 => {
            if args.get_item("ws")?.is_some() && args.get_item("msg")?.is_some() {
                handler.call((ws.clone(), fn_msg.unwrap_or_default()), Some(kwargs))
            } else if args.get_item("ws")?.is_some() {
                handler.call((ws.clone(),), Some(kwargs))
            } else if args.get_item("msg")?.is_some() {
                handler.call((fn_msg.unwrap_or_default(),), Some(kwargs))
            } else {
                handler.call((), Some(kwargs))
            }
        }
        4_u8..=u8::MAX => handler.call((ws.clone(), fn_msg.unwrap_or_default()), Some(kwargs)),
    }
}

/// Execute a WebSocket function (connect, message, or close handler).
pub fn execute_ws_function(
    function: &FunctionInfo,
    text: Option<String>,
    task_locals: &TaskLocals,
    ws: &WebSocketConnector,
) {
    if function.is_async {
        let fut = Python::with_gil(|py| {
            pyo3_asyncio::into_future_with_locals(
                task_locals,
                get_function_output(function, text, py, ws).unwrap(),
            )
            .unwrap()
        });
        let sender = ws.sender.clone();
        tokio::spawn(async move {
            let output = fut.await.unwrap();
            let result = Python::with_gil(|py| {
                output.extract::<Option<&str>>(py).unwrap().map(|s| s.to_string())
            });

            if let Some(msg) = result {
                if let Some(sender) = &sender {
                    let _ = sender.send(msg).await;
                }
            }
        });
    } else {
        Python::with_gil(|py| {
            if let Some(op) = get_function_output(function, text, py, ws)
                .unwrap()
                .extract::<Option<&str>>()
                .unwrap()
            {
                if let Some(sender) = &ws.sender {
                    // We're in sync context, spawn to send asynchronously
                    let sender = sender.clone();
                    let msg = op.to_string();
                    tokio::spawn(async move {
                        let _ = sender.send(msg).await;
                    });
                }
            }
        });
    }
}
