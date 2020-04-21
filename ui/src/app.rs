use yew::{html, Component, ComponentLink, Html, ShouldRender, Properties};

use crate::client::Client;

pub struct App {
    link: ComponentLink<Self>,
    client: Client,
}

pub enum Msg {
    Click,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let client = Client::connect().unwrap();
        App { link, client }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::Click => {}
        }
        true
    }

    fn view(&self) -> Html {
        html! {
            <div>
                <button onclick=self.link.callback(|_| Msg::Click)>{ "Click" }</button>
            </div>
        }
    }
}
