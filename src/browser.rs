//! Fixed browser-chrome operations. Never evaluate caller supplied JavaScript here.
use crate::{error::internal, ff_rdp::Backend};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

struct Rdp(TcpStream);
impl Rdp {
    async fn read(&mut self) -> Result<Value, rmcp::ErrorData> {
        let mut digits = Vec::new();
        loop {
            let c = self.0.read_u8().await.map_err(internal)?;
            if c == b':' {
                break;
            }
            if !c.is_ascii_digit() || digits.len() >= 9 {
                return Err(internal("invalid RDP frame"));
            }
            digits.push(c);
        }
        let n: usize = std::str::from_utf8(&digits)
            .map_err(internal)?
            .parse()
            .map_err(internal)?;
        if n > 8_000_000 {
            return Err(internal("RDP frame too large"));
        }
        let mut data = vec![0; n];
        self.0.read_exact(&mut data).await.map_err(internal)?;
        serde_json::from_slice(&data).map_err(internal)
    }
    async fn ask(&mut self, request: Value) -> Result<Value, rmcp::ErrorData> {
        let bytes = serde_json::to_vec(&request).map_err(internal)?;
        self.0
            .write_all(format!("{}:", bytes.len()).as_bytes())
            .await
            .map_err(internal)?;
        self.0.write_all(&bytes).await.map_err(internal)?;
        loop {
            let reply = self.read().await?;
            if reply["from"] == request["to"] && reply.get("type").is_none() {
                if reply.get("error").is_some() {
                    return Err(internal(format!("RDP: {reply}")));
                }
                return Ok(reply);
            }
        }
    }
}
impl Backend {
    pub async fn chrome(&self, expression: String) -> Result<Value, rmcp::ErrorData> {
        let work = async {
            let mut rdp = Rdp(TcpStream::connect((self.host.as_str(), self.port))
                .await
                .map_err(internal)?);
            rdp.read().await?;
            let descriptor = rdp
                .ask(json!({"to":"root","type":"getProcess","id":0}))
                .await?;
            let target = rdp
                .ask(json!({"to":descriptor["processDescriptor"]["actor"],"type":"getTarget"}))
                .await?;
            let actor = target["process"]["consoleActor"]
                .as_str()
                .ok_or_else(|| internal("missing browser console actor"))?;
            let ack = rdp
                .ask(json!({"to":actor,"type":"evaluateJSAsync","text":expression,"options":{}}))
                .await?;
            loop {
                let result = rdp.read().await?;
                if result["type"] == "evaluationResult" && result["resultID"] == ack["resultID"] {
                    if result["hasException"] == true {
                        return Err(internal(
                            "browser operation failed; inspect before retrying",
                        ));
                    }
                    let text = result["result"]
                        .as_str()
                        .ok_or_else(|| internal("invalid browser result"))?;
                    return serde_json::from_str(text).map_err(internal);
                }
            }
        };
        tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms.min(5000)),
            work,
        )
        .await
        .map_err(|_| internal("browser operation timed out; inspect before retrying"))?
    }
}
