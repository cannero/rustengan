mod writers;

use anyhow::Context;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::BufRead;
use writers::StdoutWriter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<Payload> {
    pub src: String,
    #[serde(rename = "dest")]
    pub dst: String,
    pub body: Body<Payload>,
}

impl<Payload> Message<Payload> {
    pub fn into_reply(self, id: Option<&mut usize>) -> Self {
        Self {
            src: self.dst,
            dst: self.src,
            body: Body {
                id: id.map(|id| {
                    let mid = *id;
                    *id += 1;
                    mid
                }),
                in_reply_to: self.body.id,
                payload: self.body.payload,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event<Payload, InjectedPayload = ()> {
    Message(Message<Payload>),
    Injected(InjectedPayload),
    EOF,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body<Payload> {
    #[serde(rename = "msg_id")]
    pub id: Option<usize>,
    pub in_reply_to: Option<usize>,
    #[serde(flatten)]
    pub payload: Payload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum InitPayload {
    Init(Init),
    InitOk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Init {
    pub node_id: String,
    pub node_ids: Vec<String>,
}

pub trait Node<S, Payload: std::fmt::Debug, InjectedPayload = ()> {
    fn from_init(
        state: S,
        init: Init,
        inject: std::sync::mpsc::Sender<Event<Payload, InjectedPayload>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn step<Writer>(
        &mut self,
        input: Event<Payload, InjectedPayload>,
        writer: &mut Writer,
    ) -> anyhow::Result<()>
    where
        Writer: MessageWriter<Payload>;
}

pub trait MessageWriter<Payload: std::fmt::Debug> {
    fn write(&mut self, message: &Message<Payload>) -> anyhow::Result<()>
    where
        Payload: Serialize;

    fn write_with_callback(
        &mut self,
        message: &Message<Payload>,
        callback: std::sync::mpsc::Sender<Message<Payload>>,
    ) -> anyhow::Result<()>
    where
        Payload: Serialize;

    fn callback_exists(&self, message: &Message<Payload>) -> bool;

    fn send_callback(&self, message: Message<Payload>) -> anyhow::Result<()>;
}

pub fn main_loop<S, N, P, IP>(init_state: S) -> anyhow::Result<()>
where
    P: DeserializeOwned + Send + Clone + std::fmt::Debug + 'static,
    N: Node<S, P, IP>,
    IP: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();

    let stdin = std::io::stdin().lock();
    let mut stdin = stdin.lines();
    let init = run_init(&mut stdin)?;
    let mut node: N =
        Node::from_init(init_state, init, tx.clone()).context("node initilization failed")?;

    drop(stdin);
    let stdout = std::io::stdout();
    let mut writer = StdoutWriter::new(stdout);
    let writer_callback = writer.clone();
    let jh = std::thread::spawn(move || {
        let stdin = std::io::stdin().lock();
        for line in stdin.lines() {
            let line = line.context("Maelstrom input from STDIN could not be read")?;
            //eprintln!("input from STDIN: {}", line);
            let input: Message<P> = serde_json::from_str(&line)
                .context("Maelstrom input from STDIN could not be deserialized")?;
            if writer_callback.callback_exists(&input) {
                writer_callback
                    .send_callback(input)
                    .context("could not send callback")?;
            } else {
                if tx.send(Event::Message(input)).is_err() {
                    return Ok::<_, anyhow::Error>(());
                }
            }
        }
        let _ = tx.send(Event::EOF);
        Ok(())
    });

    for input in rx {
        node.step(input, &mut writer)
            .context("Node step function failed")?;
    }

    jh.join()
        .expect("stdin thread panicked")
        .context("stdin thread err'd")?;

    Ok(())
}

fn run_init(stdin: &mut std::io::Lines<std::io::StdinLock<'_>>) -> Result<Init, anyhow::Error> {
    let stdout = std::io::stdout();
    let mut writer = StdoutWriter::new(stdout);
    let init_msg: Message<InitPayload> = serde_json::from_str(
        &stdin
            .next()
            .expect("no init message received")
            .context("failed to read init message from stdin")?,
    )
    .context("init message could not be deserialized")?;
    let InitPayload::Init(init) = init_msg.body.payload else {
        panic!("first message should be init");
    };
    let reply = Message {
        src: init_msg.dst,
        dst: init_msg.src,
        body: Body {
            id: Some(0),
            in_reply_to: init_msg.body.id,
            payload: InitPayload::InitOk,
        },
    };
    writer.write(&reply).context("send response to init")?;
    Ok(init)
}
