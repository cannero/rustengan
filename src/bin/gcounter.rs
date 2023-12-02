use rustengan::*;

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Payload {
    Add { delta: usize },
    AddOk,
    Read,
    ReadOk { value: usize },
    #[serde(rename = "read")]
    ReadKV { key: String },
    Write { key: String, value: usize },
    WriteOk,
    Error { code: usize, text: String },
}

struct GCounterNode {
    node: String,
    id: usize,
}

impl Node<(), Payload> for GCounterNode {
    fn from_init(
        _state: (),
        init: Init,
        _tx: std::sync::mpsc::Sender<Event<Payload>>,
    ) -> anyhow::Result<Self> {
        Ok(GCounterNode { 
            node: init.node_id,
            id: 1, })
    }

    fn step<Writer>(&mut self, input: Event<Payload>, writer: &mut Writer) -> anyhow::Result<()> 
    where Writer: MessageWriter<Payload>{
        let Event::Message(input) = input else {
            panic!("got injected event when there's no event injection");
        };
        
        let mut reply = input.into_reply(Some(&mut self.id));
        match reply.body.payload {
            Payload::Add { delta: _ } => {
                reply.body.payload = Payload::AddOk;
                writer.write(&reply).context("send response to echo")?;

                let save_message = Message{
                    src: self.node.clone(),
                    dst: "seq-kv".to_string(),
                    body: Body {
                        id: Some(self.id),
                        payload: Payload::Write { key: "counter".to_string(), value: 998877 },
                        in_reply_to: None,
                    }
                };
                writer.write(&save_message).context("send write to seq-kv")?;
                self.id += 1;

            }
            Payload::AddOk => {}
            Payload::Read => {

                let read_message = Message{
                    src: self.node.clone(),
                    dst: "seq-kv".to_string(),
                    body: Body {
                        id: Some(self.id),
                        payload: Payload::ReadKV { key: "counter".to_string() },
                        in_reply_to: None,
                    }
                };
                self.id += 1;

                let (tx_read, rx_read) = std::sync::mpsc::channel();
                writer.write_with_callback(&read_message, tx_read)?;

                // TODO: use a timeout, what happens with Sender?
                let read_result = rx_read.recv().context("read response from seq-kv")?;

                let value = if let Payload::ReadOk { value } = read_result.body.payload {
                    value
                } else {
                    0
                };

                reply.body.payload = Payload::ReadOk { value };
                writer.write(&reply).context("send read ok")?;
            }
            Payload::ReadOk { .. } => {}
            Payload::ReadKV { .. } => {}
            Payload::Write { .. } => {}
            Payload::WriteOk => {}
            Payload::Error { .. } => panic!("got error, should be handled by KV logic"),
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<_, GCounterNode, _, _>(())
}
