use anyhow::Context;
use serde::Serialize;
use std::{
    collections::HashMap,
    io::{Stdout, Write},
    sync::{Arc, Mutex},
};

use crate::{Message, MessageWriter};

#[derive(Debug, Clone)]
pub struct StdoutWriter<Payload>
where
    Payload: Clone,
{
    output: Arc<Mutex<Stdout>>,
    callbacks: Arc<Mutex<HashMap<usize, std::sync::mpsc::Sender<Message<Payload>>>>>,
}

impl<'a, Payload: Clone> StdoutWriter<Payload> {
    pub fn new(output: Stdout) -> Self {
        Self {
            output: Arc::new(Mutex::new(output)),
            callbacks: Default::default(),
        }
        .clone()
    }
}

impl<Payload: Clone + std::fmt::Debug> MessageWriter<Payload> for StdoutWriter<Payload> {
    fn write(&mut self, message: &Message<Payload>) -> anyhow::Result<()>
    where
        Payload: Serialize + Clone,
    {
        //eprintln!("writing {:?}", message.body);
        let mut output = self.output.lock().unwrap();
        serde_json::to_writer(&mut *output, message).context("serialize response")?;
        output.write_all(b"\n").context("trailling newline")?;
        Ok(())
    }

    fn write_with_callback(
        &mut self,
        message: &Message<Payload>,
        callback: std::sync::mpsc::Sender<Message<Payload>>,
    ) -> anyhow::Result<()>
    where
        Payload: Serialize,
    {
        match message.body.id {
            Some(id) => {
                self.callbacks.lock().unwrap().insert(id, callback);
                self.write(&message)
            }
            None => panic!("Message has no id"),
        }
    }

    fn callback_exists(&self, message: &Message<Payload>) -> bool {
        match message.body.in_reply_to {
            Some(id) => self.callbacks.lock().unwrap().contains_key(&id),
            None => false,
        }
    }

    fn send_callback(&self, message: Message<Payload>) -> anyhow::Result<()> {
        if self.callback_exists(&message) {
            let callback = self
                .callbacks
                .lock()
                .unwrap()
                .remove(&message.body.in_reply_to.unwrap())
                .unwrap();
            _ = callback.send(message);
        }
        Ok(())
    }
}
