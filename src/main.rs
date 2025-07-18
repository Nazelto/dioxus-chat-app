use daisy_rsx::{Card, CardBody, CardHeader, Fieldset};
use dioxus::prelude::{
    server_fn::codec::{StreamingText, TextStream},
    *,
};
use futures::StreamExt;
use gset::Getset;
pub static MONDB_URL: &str =
    "mongodb://mongo:mongo@localhost:27017/?maxPoolSize=20&w=majority&replicaSet=rs0&directConnection=true";
fn main() {
    dioxus::launch(App);
}
#[allow(clippy::redundant_closure)]
#[component]
fn App() -> Element {
    let mut message = use_signal(|| String::new());
    let mut messages = use_signal(|| Vec::<Message>::new());
    let mut input_messages = use_signal(|| Vec::<Message>::new());
    use_future(move || async move {
        for msg in get_message().await.unwrap().into_iter() {
            if !msg.message_as_ref().trim().is_empty() {
                messages.write().push(msg);
            };
        }
    });
    use_effect(move || {
        spawn(async move {
            let stream = message_stream()
                .await
                .inspect_err(|e| {
                    eprintln!("获取文本流时出现错误: {}", e);
                })
                .unwrap();
            let mut stream = stream.into_inner();
            while let Some(Ok(accept_message_str)) = stream.next().await {
                let accept_message = serde_json::from_str::<Message>(&accept_message_str).unwrap();
                if !accept_message.message_as_ref().trim().is_empty()
                    || accept_message.message_as_ref().trim() != ""
                {
                    println!("{:#?}", accept_message);
                    input_messages.write().push(accept_message);
                }
            }
        });
    });
    rsx! {
        document::Link {
            rel: "stylesheet",
            href: asset!("assets/tailwind.css"),
        }
        //TODO: update component
        div {
            class: "bg-white w-[100vw] h-[100vh] flex justify-center items-center ",
            Card {
                class: "bg-gray-100 p-3 shadow-md w-[50%] h-[50%] flex flex-col items-center",
                CardHeader {
                    class: "text-black antialiased font-light font-[JetBrainsMonoNL Nerd Font Mono] tracking-wider",
                    title: "Message",
                }
                div {
                    class: "divider divider-primary",
                }
                CardBody {
                    class: "w-[100%]",
                    nav {
                        class: "overflow-auto flex min-w-[240px] flex-col gap-1 p-1.5",
                        for message in messages.iter() {
                            div {
                                class: "chat chat-start",
                                div {
                                    class: "chat-bubble ",
                                    span {
                                        class: "font-light font-[JetBrainsMonoNL Nerd Font Mono] underline decoration-sky-500 underline-offset-4",
                                        {message.message_as_ref()}
                                    }
                                }
                            }
                        }
                        for message in input_messages.iter() {
                            div {
                                class: "chat chat-end",
                                div {
                                    class: "chat-bubble",
                                    {message.message_as_ref()}
                                }
                            }
                        }
                    }
                }
                div {
                    class: "divider divider-primary",
                }
                div {
                    class: " w-[80%] flex justify-center items-center h-[30%] mb-[10px]",
                    div {
                        class: "relative w-full flex justify-center items-center min-w-[200px] h-[50%]",
                        //TODO: FIX Cardfooter
                        Fieldset {
                            legend: "Please input content.",
                            legend_class: "text-gray-600",
                            input {
                                class: "input input-primary text-black shadow-md bg-gray-300 focus:bg-gray-500 focus:text-white",
                                r#type: "text",
                                placeholder: "please input content",
                                oninput: move |e| {
                                    message.set(e.value());
                                },
                                                        //label: "messages",
                            //label_class: "text-black",
                            }
                            p {
                                class: "label text-gray-300",
                                "Optional"
                            }
                        }
                    }
                    button {
                        class: "width:fit-content",
                        onclick: move |_| async move {
                            send_message(message()).await.unwrap();
                        },
                        class: "btn",
                        "Send"
                    }

                }
            }
        }
    }
}
#[server(output = StreamingText)]
pub async fn message_stream() -> Result<TextStream, ServerFnError> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    tokio::spawn(async move {
        //连接mongodb
        let client = mongodb::Client::with_uri_str(MONDB_URL)
            .await
            .inspect_err(|e| {
                eprintln!("连接mongodb时出现错误:{}", e);
            })
            .unwrap_or_else(|_| {
                std::process::exit(1);
            });
        let db = client.database("dioxus");
        let col = db.collection::<Message>("messages");
        let mut stream = col
            .watch()
            .await
            .inspect_err(|e| {
                eprintln!("连接时出现错误:{}", e);
            })
            .unwrap_or_else(|_| {
                std::process::exit(1);
            });
        while let Some(change) = stream.next().await {
            if let Ok(event) = change {
                if let Some(full_doc) = event.full_document {
                    let json = serde_json::to_string(&full_doc).unwrap();
                    tx.unbounded_send(Ok(json)).unwrap();
                }
            }
        }
    });
    Ok(TextStream::new(rx))
}
#[server]
pub async fn send_message(message: String) -> Result<(), ServerFnError> {
    let client = mongodb::Client::with_uri_str(MONDB_URL)
        .await
        .inspect_err(|e| {
            eprintln!("连接时发生错误:{}", e);
        })
        .unwrap_or_else(|_| {
            std::process::exit(1);
        });
    let db = client.database("dioxus");
    let col = db.collection::<Message>("messages");
    col.insert_one(Message::default().message(message)).await?;
    Ok(())
}
#[server]
pub async fn get_message() -> Result<Vec<Message>, ServerFnError> {
    let client = try_connect_mongodb().await.unwrap();
    let db = client.database("dioxus");
    let mut cursor = db
        .collection::<Message>("messages")
        .find(mongodb::bson::doc! {})
        .await
        .unwrap();
    let mut messages = Vec::new();
    while let Some(Ok(message)) = cursor.next().await {
        messages.push(message);
    }
    Ok(messages)
}
#[cfg(feature = "server")]
pub async fn try_connect_mongodb() -> anyhow::Result<mongodb::Client> {
    Ok(mongodb::Client::with_uri_str(MONDB_URL)
        .await
        .inspect_err(|e| {
            eprintln!("连接mongodb时出现错误:{}", e);
        })?)
}
#[derive(Debug, serde::Serialize, serde::Deserialize, Default, Getset)]
pub struct Message {
    #[getset(set_own, name = "message", vis = "pub")]
    #[getset(get_as_ref, name = "message_as_ref", vis = "pub", ty = "&str")]
    message: String,
}
#[cfg(test)]
mod tests {
    use crate::{send_message, MONDB_URL};
    #[test]
    fn test_connect() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .worker_threads(8)
            .build()
            .unwrap();
        rt.block_on(async move {
            mongodb::Client::with_uri_str(MONDB_URL)
                .await
                .inspect(|_| {
                    println!("connect success!");
                })
                .inspect_err(|e| {
                    eprintln!("occur error:{}", e);
                })
                .unwrap();
        });
        println!("end.");
    }
    #[test]
    fn test_insert_one_to_mongodb() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .worker_threads(8)
            .build()
            .unwrap();
        rt.block_on(async move {
            send_message("这是一个测试".to_string()).await.unwrap();
        });
    }
}
