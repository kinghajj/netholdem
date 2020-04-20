#![warn(rust_2018_idioms)]

use futures::channel::mpsc;
use js_sys::Uint8Array;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, ErrorEvent, MessageEvent, WebSocket};

use netholdem_game::protocol::{IntroductionRequest, Request, Response};

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

fn connect() -> Result<WebSocket, JsValue> {
    let window = web_sys::window().expect("window to exist");
    let addr = format!(
        "ws://{}:{}/server",
        window.location().hostname().expect("hostname to exist"),
        window.location().port().expect("port to exist")
    );
    let ws = WebSocket::new(&addr)?;
    Ok(ws)
}

#[wasm_bindgen]
pub struct Client {
    ws: WebSocket,
    state: Rc<RefCell<State>>,
}

#[wasm_bindgen]
impl Client {
    pub fn connect() -> Result<Client, JsValue> {
        let (request_tx, _request_rx) = mpsc::channel(8);
        let (_response_tx, response_rx) = mpsc::channel(8);
        let state = Rc::new(RefCell::new(State::new(request_tx, response_rx)));
        let ws = connect()?;
        ws.set_binary_type(BinaryType::Arraybuffer);

        let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
            console_log!("raw response: {:?}", e.data());
            let msg = Uint8Array::new(&e.data()).to_vec();
            console_log!("vec response: {:?}", msg);
            let response: Result<Response, _> = bincode::deserialize(&msg);
            match response {
                Ok(response) => {
                    console_log!("got response: {:?}", response);
                }
                Err(e) => {
                    console_log!("error decoding response: {}", e);
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
            console_log!("error event: {:?}", e);
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let cloned_ws = ws.clone();
        let onopen_callback = Closure::wrap(Box::new(move |_| {
            console_log!("socket opened");
            let request = Request::Introduction(IntroductionRequest::default());
            let request_bytes: Vec<u8> =
                bincode::serialize(&request).expect("serialization to work");
            match cloned_ws.send_with_u8_array(&request_bytes) {
                Ok(_) => console_log!("message successfully sent"),
                Err(err) => console_log!("error sending message: {:?}", err),
            }
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        let client = Client { ws, state };
        Ok(client)
    }
}

struct State {
    request_tx: mpsc::Sender<Request>,
    response_rx: mpsc::Receiver<Response>,
}

impl State {
    pub fn new(request_tx: mpsc::Sender<Request>, response_rx: mpsc::Receiver<Response>) -> Self {
        State {
            request_tx,
            response_rx,
        }
    }
}
