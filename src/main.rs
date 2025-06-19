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
                    messages.write().push(accept_message);
                }
            }
        });
    });
    rsx! {
        document::Link {
            rel: "stylesheet",
            href: asset!("assets/tailwind.css"),
        }
        div {
            class: " w-[100vw] h-[100vh] flex justify-center items-center ",
            div {
                class: "relative flex flex-col justify-between items-center my-6 bg-white shadow-sm border border-slate-200 rounded-lg w-[50%] h-[50%]",
                div {
                    class: "mx-3 mb-0 border-b border-slate-200 pt-3 pb-2 px-1 w-full relative h-[15%] flex items-center",
                    span {
                        class: "text-md text-slate-600 font-medium ",
                        "Message"
                    }
                }
                nav {
                    class: "overflow-auto flex min-w-[240px] flex-col gap-1 p-1.5",
                    for message in messages.iter() {
                        div {
                            class: "text-slate-800 flex w-full items-center rounded-md p-3 transition-all hover:bg-slate-100 focus:bg-slate-100 active:bg-slate-100",
                            span {
                                class: "block text-md text-gray-700 leading-relaxed",
                                {message.message_as_ref()}
                            }
                        }
                    }
                }
                div {
                    class: "w-96 flex items-center h-[30%] mb-[10px]",
                    div {
                        class: "relative w-full min-w-[200px] h-[50%]",
                        textarea {
                            placeholder: "",
                            class: "peer h-full w-full resize-none border-b border-blue-gray-200 bg-transparent pt-4 pb-1.5 font-sans text-sm font-normal text-blue-gray-700 outline outline-0 transition-all placeholder-shown:border-blue-gray-200 focus:border-gray-900 focus:outline-0 disabled:resize-none disabled:border-0 disabled:bg-blue-gray-50",
                            oninput: move |e| {
                                message.set(e.value());
                            },
                        }
                        label {
                            class: "after:content[' '] pointer-events-none absolute left-0 -top-1.5 flex h-full w-full select-none text-[11px] font-normal leading-tight text-blue-gray-500 transition-all after:absolute after:-bottom-0 after:block after:w-full after:scale-x-0 after:border-b-2 after:border-gray-900 after:transition-transform after:duration-300 peer-placeholder-shown:text-sm peer-placeholder-shown:leading-[4.25] peer-placeholder-shown:text-blue-gray-500 peer-focus:text-[11px] peer-focus:leading-tight peer-focus:text-gray-900 peer-focus:after:scale-x-100 peer-focus:after:border-gray-900 peer-disabled:text-transparent peer-disabled:peer-placeholder-shown:text-blue-gray-500",
                            "Message"
                        }
                    }
                    button {
                        class: "ml-[10px] inline-flex text-white bg-indigo-500 border-0 py-2 px-6 focus:outline-hidden hover:bg-indigo-600 rounded-sm text-lg",
                        onclick: move |_| async move {
                            send_message(message()).await.unwrap();
                        },
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
