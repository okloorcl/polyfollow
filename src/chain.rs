use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tiny_keccak::{Hasher, Keccak};

use crate::cli::WatchChainArgs;

const ORDER_FILLED_SIG: &str =
    "OrderFilled(bytes32,address,address,uint256,uint256,uint256,uint256,uint256)";
const ORDER_FILLED_V2_SIG: &str =
    "OrderFilled(bytes32,address,address,uint8,uint256,uint256,uint256,uint256,bytes32,bytes32)";

#[derive(Clone)]
struct EvmRpc {
    url: String,
    http: Client,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvmLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    #[serde(rename = "blockNumber")]
    pub block_number: String,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    #[serde(rename = "logIndex")]
    pub log_index: String,
}

#[derive(Debug, Serialize)]
struct LogFilter<'a> {
    address: &'a str,
    #[serde(rename = "fromBlock")]
    from_block: String,
    #[serde(rename = "toBlock")]
    to_block: String,
    topics: Vec<String>,
}

impl EvmRpc {
    fn new(url: String) -> Self {
        Self {
            url,
            http: Client::new(),
        }
    }

    async fn block_number(&self) -> Result<u64> {
        let value = self.call("eth_blockNumber", json!([])).await?;
        hex_u64(value.as_str().context("block number must be hex string")?)
    }

    async fn logs(
        &self,
        address: &str,
        topic0: &str,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<EvmLog>> {
        let filter = LogFilter {
            address,
            from_block: hex_quantity(from_block),
            to_block: hex_quantity(to_block),
            topics: vec![topic0.to_string()],
        };
        let value = self.call("eth_getLogs", json!([filter])).await?;
        serde_json::from_value(value).context("failed to decode eth_getLogs response")
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let response = self
            .http
            .post(&self.url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params,
            }))
            .send()
            .await
            .context("rpc request failed")?
            .error_for_status()
            .context("rpc returned http error")?
            .json::<Value>()
            .await
            .context("rpc returned invalid json")?;

        if let Some(error) = response.get("error") {
            bail!("rpc error for {method}: {error}");
        }
        response
            .get("result")
            .cloned()
            .context("rpc response missing result")
    }
}

pub async fn watch_chain(args: WatchChainArgs, json_output: bool) -> Result<()> {
    validate_args(&args)?;
    let topics = if args.topics.is_empty() {
        order_filled_topics()
    } else {
        args.topics
    };
    let rpc = EvmRpc::new(args.rpc_url);
    let mut from_block = args.from_block.unwrap_or(rpc.block_number().await?);
    loop {
        let latest = rpc.block_number().await?;
        if from_block <= latest {
            let to_block = latest.min(from_block + args.batch_blocks.saturating_sub(1));
            let logs = fetch_logs(&rpc, &args.contracts, &topics, from_block, to_block).await?;
            print_logs(&logs, json_output)?;
            from_block = to_block + 1;
        }
        if args.once {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(args.poll_secs)).await;
    }
    Ok(())
}

fn validate_args(args: &WatchChainArgs) -> Result<()> {
    if args.contracts.is_empty() {
        bail!("provide at least one --contract");
    }
    if args.batch_blocks == 0 {
        bail!("--batch-blocks must be greater than zero");
    }
    if args.poll_secs == 0 {
        bail!("--poll-secs must be greater than zero");
    }
    Ok(())
}

async fn fetch_logs(
    rpc: &EvmRpc,
    contracts: &[String],
    topics: &[String],
    from_block: u64,
    to_block: u64,
) -> Result<Vec<EvmLog>> {
    let mut out = Vec::new();
    for contract in contracts {
        for topic in topics {
            out.extend(rpc.logs(contract, topic, from_block, to_block).await?);
        }
    }
    Ok(out)
}

fn print_logs(logs: &[EvmLog], json_output: bool) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string(logs)?);
    } else if logs.is_empty() {
        println!("chain: no logs");
    } else {
        for log in logs {
            println!(
                "chain: block={} tx={} log_index={} address={}",
                log.block_number, log.transaction_hash, log.log_index, log.address
            );
        }
    }
    Ok(())
}

fn order_filled_topics() -> Vec<String> {
    vec![
        event_topic(ORDER_FILLED_SIG),
        event_topic(ORDER_FILLED_V2_SIG),
    ]
}

fn event_topic(signature: &str) -> String {
    let mut hasher = Keccak::v256();
    let mut out = [0u8; 32];
    hasher.update(signature.as_bytes());
    hasher.finalize(&mut out);
    format!("0x{}", hex_encode(&out))
}

fn hex_u64(value: &str) -> Result<u64> {
    Ok(u64::from_str_radix(value.trim_start_matches("0x"), 16)?)
}

fn hex_quantity(value: u64) -> String {
    format!("0x{value:x}")
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_filled_topic_is_stable() {
        assert_eq!(event_topic(ORDER_FILLED_SIG).len(), 66);
        assert_ne!(
            event_topic(ORDER_FILLED_SIG),
            event_topic(ORDER_FILLED_V2_SIG)
        );
    }

    #[test]
    fn watch_chain_rejects_zero_intervals() {
        let mut args = WatchChainArgs {
            rpc_url: "https://example.invalid".to_string(),
            contracts: vec!["0x2222222222222222222222222222222222222222".to_string()],
            topics: Vec::new(),
            from_block: Some(1),
            batch_blocks: 1000,
            poll_secs: 5,
            once: true,
        };
        args.batch_blocks = 0;

        assert!(validate_args(&args).is_err());

        args.batch_blocks = 1000;
        args.poll_secs = 0;

        assert!(validate_args(&args).is_err());
    }
}
