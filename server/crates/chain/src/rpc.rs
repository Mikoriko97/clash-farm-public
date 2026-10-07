//! JSON-RPC client with two providers, 10 s timeout, 1 MB response cap and
//! bounded retries. Responses are untrusted input (SECURITY §4).

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::error::ChainError;
use crate::parsed::{AccountInfoResult, MintInfo, RpcEnvelope, TokenAccountInfo, TransactionResult};

const MAX_BODY_BYTES: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);
const RETRIES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commitment {
    Confirmed,
    Finalized,
}

impl Commitment {
    fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Finalized => "finalized",
        }
    }
}

#[derive(Clone)]
pub struct RpcConfig {
    /// URLs may embed provider keys: never log them.
    pub primary_url: String,
    pub secondary_url: Option<String>,
}

impl std::fmt::Debug for RpcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcConfig")
            .field("secondary", &self.secondary_url.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct RpcClient {
    http: reqwest::Client,
    config: RpcConfig,
}

impl RpcClient {
    pub fn new(config: RpcConfig) -> Result<Self, ChainError> {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ChainError::Transport(e.to_string()))?;
        Ok(Self { http, config })
    }

    /// `getTransaction(jsonParsed)`; `Ok(None)` when the RPC does not know the signature yet.
    pub async fn get_transaction(
        &self,
        signature: &str,
        commitment: Commitment,
    ) -> Result<Option<TransactionResult>, ChainError> {
        let params = json!([
            signature,
            { "encoding": "jsonParsed", "commitment": commitment.as_str(), "maxSupportedTransactionVersion": 0 }
        ]);
        self.call::<Option<TransactionResult>>("getTransaction", params)
            .await
    }

    /// Both providers must agree; a disagreement is reported so the order goes to review.
    pub async fn get_transaction_cross_checked(
        &self,
        signature: &str,
        commitment: Commitment,
    ) -> Result<Option<TransactionResult>, ChainError> {
        let primary = self
            .call_on(
                &self.config.primary_url,
                "getTransaction",
                tx_params(signature, commitment),
            )
            .await;
        let Some(secondary_url) = &self.config.secondary_url else {
            return primary;
        };
        let secondary = self
            .call_on(secondary_url, "getTransaction", tx_params(signature, commitment))
            .await;
        match (primary, secondary) {
            // One provider not seeing the signature yet is ordinary lag → "not visible" (client retries).
            (Ok(None), Ok(Some(_))) | (Ok(Some(_)), Ok(None)) => Ok(None),
            (Ok(a), Ok(b)) => {
                if summary(&a) == summary(&b) {
                    Ok(a)
                } else {
                    Err(ChainError::Shape(
                        "providers disagree on the transaction".to_owned(),
                    ))
                }
            }
            (Ok(a), Err(e)) | (Err(e), Ok(a)) => {
                tracing::warn!(error = %e, "one rpc provider failed; using the other");
                Ok(a)
            }
            (Err(_), Err(_)) => Err(ChainError::AllProvidersFailed),
        }
    }

    pub async fn get_token_account(&self, account: &str) -> Result<Option<TokenAccountInfo>, ChainError> {
        let result: AccountInfoResult = self
            .call("getAccountInfo", json!([account, { "encoding": "jsonParsed" }]))
            .await?;
        let Some(value) = result.value else {
            return Ok(None);
        };
        if value.data.parsed.kind != "account" {
            return Err(ChainError::Shape("not a token account".to_owned()));
        }
        let info = value.data.parsed.info;
        let mint = info
            .get("mint")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChainError::Shape("mint".into()))?;
        let owner = info
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChainError::Shape("owner".into()))?;
        Ok(Some(TokenAccountInfo {
            mint: mint.to_owned(),
            owner: owner.to_owned(),
        }))
    }

    pub async fn get_mint(&self, mint: &str) -> Result<Option<MintInfo>, ChainError> {
        let result: AccountInfoResult = self
            .call("getAccountInfo", json!([mint, { "encoding": "jsonParsed" }]))
            .await?;
        let Some(value) = result.value else {
            return Ok(None);
        };
        if value.data.parsed.kind != "mint" {
            return Err(ChainError::Shape("not a mint".to_owned()));
        }
        let info = value.data.parsed.info;
        let decimals = info
            .get("decimals")
            .and_then(|v| v.as_u64())
            .and_then(|d| u8::try_from(d).ok())
            .ok_or_else(|| ChainError::Shape("decimals".into()))?;
        let mint_authority = info
            .get("mintAuthority")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        Ok(Some(MintInfo {
            decimals,
            mint_authority,
        }))
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, ChainError> {
        match self
            .call_on(&self.config.primary_url, method, params.clone())
            .await
        {
            Ok(value) => Ok(value),
            Err(primary_err) => match &self.config.secondary_url {
                Some(url) => {
                    tracing::warn!(error = %primary_err, "primary rpc failed; trying secondary");
                    self.call_on(url, method, params).await
                }
                None => Err(primary_err),
            },
        }
    }

    async fn call_on<T: DeserializeOwned>(
        &self,
        url: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, ChainError> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let mut last = ChainError::Transport("no attempt".to_owned());
        for attempt in 0..RETRIES {
            match self.once::<T>(url, &body).await {
                Ok(value) => return Ok(value),
                Err(err @ ChainError::Transport(_)) => {
                    last = err;
                    tokio::time::sleep(Duration::from_millis(200 * (1 << attempt))).await;
                }
                Err(err) => return Err(err),
            }
        }
        Err(last)
    }

    async fn once<T: DeserializeOwned>(&self, url: &str, body: &serde_json::Value) -> Result<T, ChainError> {
        let mut response = self
            .http
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| ChainError::Transport(redact(&e.to_string())))?;
        if response
            .content_length()
            .is_some_and(|len| len > MAX_BODY_BYTES as u64)
        {
            return Err(ChainError::TooLarge);
        }
        // Stream with a cap: a chunked body without Content-Length must not be buffered whole.
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| ChainError::Transport(redact(&e.to_string())))?
        {
            if bytes.len() + chunk.len() > MAX_BODY_BYTES {
                return Err(ChainError::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        let envelope: RpcEnvelope<serde_json::Value> =
            serde_json::from_slice(&bytes).map_err(|e| ChainError::Shape(e.to_string()))?;
        if let Some(err) = envelope.error {
            return Err(ChainError::Rpc {
                code: err.code,
                message: err.message,
            });
        }
        // `result: null` is a valid answer (unknown signature) and must reach `T = Option<_>`.
        serde_json::from_value::<T>(envelope.result.unwrap_or(serde_json::Value::Null))
            .map_err(|e| ChainError::Shape(e.to_string()))
    }
}

fn tx_params(signature: &str, commitment: Commitment) -> serde_json::Value {
    json!([
        signature,
        { "encoding": "jsonParsed", "commitment": commitment.as_str(), "maxSupportedTransactionVersion": 0 }
    ])
}

type InstructionDigest = (String, Option<serde_json::Value>);
type TxDigest = (
    u64,
    Option<i64>,
    Option<serde_json::Value>,
    Vec<InstructionDigest>,
);

/// Everything `verify` reads, so a provider that alters mint/authority/amount/
/// destination (outer or inner) is caught as a disagreement.
fn summary(tx: &Option<TransactionResult>) -> Option<TxDigest> {
    tx.as_ref().map(|t| {
        let inner = t
            .meta
            .iter()
            .flat_map(|m| m.inner_instructions.iter())
            .flat_map(|i| i.instructions.iter());
        let instructions = t
            .transaction
            .message
            .instructions
            .iter()
            .chain(inner)
            .map(|ix| (ix.program_id.clone(), ix.parsed.clone()))
            .collect();
        (
            t.slot,
            t.block_time,
            t.meta.as_ref().and_then(|m| m.err.clone()),
            instructions,
        )
    })
}

/// Error strings may echo the URL (with an API key): keep only the kind of failure.
fn redact(message: &str) -> String {
    message
        .split(" for url")
        .next()
        .unwrap_or("transport error")
        .to_owned()
}
