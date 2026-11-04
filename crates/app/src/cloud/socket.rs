//! The sync WebSocket on every target: tokio-tungstenite on the Mac and
//! Android, the browser's `WebSocket` on the web. Browsers can't set an
//! `Authorization` header on a WebSocket, so the web build offers the token
//! as a subprotocol (`kabar.v1`, `kabar.bearer.<token>`), which Kabar Cloud
//! accepts on `/v1/sync` (crates/server/src/web.rs).

/// What the socket heard.
pub enum Frame {
    Text(String),
    /// The server closed it, with this close code.
    Closed(Option<u16>),
    /// A frame the engine doesn't use (binary, pong).
    Other,
    Error(String),
}

/// Why the socket didn't open.
pub enum ConnectError {
    /// The server refused the token (native only: browsers don't show the
    /// handshake's status, so the web reports [`ConnectError::Failed`]).
    Unauthorized,
    Failed(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => f.write_str("401 Unauthorized"),
            Self::Failed(e) => f.write_str(e),
        }
    }
}

pub use imp::Socket;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::{Error, Message};
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

    use super::{ConnectError, Frame};

    pub struct Socket(WebSocketStream<MaybeTlsStream<TcpStream>>);

    impl Socket {
        pub async fn connect(url: &str, access: &str) -> Result<Self, ConnectError> {
            let mut request =
                url.into_client_request().map_err(|e| ConnectError::Failed(format!("bad sync url: {e}")))?;
            if let Ok(v) = format!("Bearer {access}").parse() {
                request.headers_mut().insert("authorization", v);
            }
            match tokio_tungstenite::connect_async(request).await {
                Ok((ws, _)) => Ok(Self(ws)),
                Err(Error::Http(res)) if res.status().as_u16() == 401 => Err(ConnectError::Unauthorized),
                Err(e) => Err(ConnectError::Failed(e.to_string())),
            }
        }

        pub async fn next(&mut self) -> Frame {
            match self.0.next().await {
                Some(Ok(Message::Text(text))) => Frame::Text(text.to_string()),
                Some(Ok(Message::Close(close))) => Frame::Closed(close.map(|c| u16::from(c.code))),
                Some(Ok(_)) => Frame::Other,
                Some(Err(e)) => Frame::Error(e.to_string()),
                None => Frame::Closed(None),
            }
        }

        pub async fn send(&mut self, text: String) -> Result<(), String> {
            self.0.send(Message::Text(text.into())).await.map_err(|e| e.to_string())
        }

        /// Keeps NAT mappings and the Cloudflare edge from dropping an idle
        /// socket.
        pub async fn ping(&mut self) -> Result<(), String> {
            self.0.send(Message::Ping(Vec::new().into())).await.map_err(|e| e.to_string())
        }

        pub async fn close(mut self) {
            let _ = self.0.close(None).await;
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use kabar_protocol::sync::{WS_BEARER_PREFIX, WS_PROTOCOL};
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::Closure;
    use web_sys::{CloseEvent, Event, MessageEvent, WebSocket};

    use super::{ConnectError, Frame};

    enum Heard {
        Open,
        Frame(Frame),
    }

    pub struct Socket {
        ws: WebSocket,
        heard: UnboundedReceiver<Heard>,
        // Kept alive as long as the socket: the browser calls them.
        _handlers: [Closure<dyn FnMut(Event)>; 4],
    }

    impl Socket {
        pub async fn connect(url: &str, access: &str) -> Result<Self, ConnectError> {
            let protocols =
                js_sys::Array::of2(&WS_PROTOCOL.into(), &format!("{WS_BEARER_PREFIX}{access}").into());
            let ws = WebSocket::new_with_str_sequence(url, &protocols)
                .map_err(|e| ConnectError::Failed(format!("{e:?}")))?;
            let (tx, heard) = unbounded_channel();
            let on = |f: Box<dyn FnMut(Event)>| Closure::wrap(f);
            let open = {
                let tx = tx.clone();
                on(Box::new(move |_| {
                    tx.send(Heard::Open).ok();
                }))
            };
            let message = {
                let tx = tx.clone();
                on(Box::new(move |e: Event| {
                    let frame = match e.dyn_into::<MessageEvent>().ok().and_then(|m| m.data().as_string()) {
                        Some(text) => Frame::Text(text),
                        None => Frame::Other,
                    };
                    tx.send(Heard::Frame(frame)).ok();
                }))
            };
            let close = {
                let tx = tx.clone();
                on(Box::new(move |e: Event| {
                    let code = e.dyn_into::<CloseEvent>().ok().map(|c| c.code());
                    tx.send(Heard::Frame(Frame::Closed(code))).ok();
                }))
            };
            let error = on(Box::new(move |_| {
                tx.send(Heard::Frame(Frame::Error("the sync socket failed".into()))).ok();
            }));
            ws.set_onopen(Some(open.as_ref().unchecked_ref()));
            ws.set_onmessage(Some(message.as_ref().unchecked_ref()));
            ws.set_onclose(Some(close.as_ref().unchecked_ref()));
            ws.set_onerror(Some(error.as_ref().unchecked_ref()));
            let mut socket = Self { ws, heard, _handlers: [open, message, close, error] };
            match socket.heard.recv().await {
                Some(Heard::Open) => Ok(socket),
                // A refused handshake looks like any other failure here.
                Some(Heard::Frame(Frame::Error(e))) => Err(ConnectError::Failed(e)),
                _ => Err(ConnectError::Failed("the sync socket closed while opening".into())),
            }
        }

        pub async fn next(&mut self) -> Frame {
            match self.heard.recv().await {
                Some(Heard::Frame(frame)) => frame,
                Some(Heard::Open) => Frame::Other,
                None => Frame::Closed(None),
            }
        }

        pub async fn send(&mut self, text: String) -> Result<(), String> {
            self.ws.send_with_str(&text).map_err(|e| format!("{e:?}"))
        }

        /// Browsers answer the server's pings themselves and can't send one.
        pub async fn ping(&mut self) -> Result<(), String> {
            Ok(())
        }

        pub async fn close(self) {
            // Drop does it.
        }
    }

    impl Drop for Socket {
        fn drop(&mut self) {
            self.ws.set_onopen(None);
            self.ws.set_onmessage(None);
            self.ws.set_onclose(None);
            self.ws.set_onerror(None);
            let _ = self.ws.close();
        }
    }
}
