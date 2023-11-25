use std::io::{StdoutLock, Write};
use anyhow::Context;
use serde::Serialize;

use crate::MessageWriter;
pub struct StdoutWriter<'a> {
    output: StdoutLock<'a>,
}

impl<'a> StdoutWriter<'a> {
    pub fn new(output: StdoutLock<'a>) -> Self {
        Self{output,}
    }
}

impl<'a, M> MessageWriter<M> for StdoutWriter<'a> {
    fn write(&mut self, message: &M) -> anyhow::Result<()>
    where
        M: Serialize,
    {
        serde_json::to_writer(&mut self.output, message).context("serialize response")?;
        self.output.write_all(b"\n").context("trailling newline")?;
        Ok(())
    }
}

