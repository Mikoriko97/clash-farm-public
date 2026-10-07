//! Strict shapes for the parts of `getTransaction(jsonParsed)` and
//! `getAccountInfo(jsonParsed)` we rely on. Everything else is ignored;
//! anything we need that is missing or oddly typed fails deserialisation
//! (fail-closed → `needs_review`).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RpcEnvelope<T> {
    pub result: Option<T>,
    pub error: Option<RpcErrorBody>,
}

#[derive(Debug, Deserialize)]
pub struct RpcErrorBody {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResult {
    pub slot: u64,
    pub block_time: Option<i64>,
    pub meta: Option<Meta>,
    pub transaction: Transaction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// Required key (may be null): an RPC answer without `err` is not a success.
    #[serde(deserialize_with = "required_nullable")]
    pub err: Option<serde_json::Value>,
    #[serde(default)]
    pub inner_instructions: Vec<InnerInstructions>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InnerInstructions {
    #[serde(default)]
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    pub message: Message,
    #[serde(default)]
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub instructions: Vec<Instruction>,
}

/// One instruction as `jsonParsed` renders it: parsed (known program) or raw.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instruction {
    pub program_id: String,
    pub program: Option<String>,
    /// Object for token instructions, plain string for memo, absent for raw.
    pub parsed: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInstruction {
    #[serde(rename = "type")]
    pub kind: String,
    pub info: TokenInfo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub mint: Option<String>,
    pub authority: Option<String>,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub account: Option<String>,
    pub token_amount: Option<TokenAmount>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenAmount {
    /// Atomic units as a decimal string (RPC never sends floats here).
    pub amount: String,
    pub decimals: u8,
}

/// `Option` fields are implicitly optional in serde; this makes the key mandatory while allowing `null`.
fn required_nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
    Option::deserialize(d)
}

impl Instruction {
    pub fn token(&self) -> Option<TokenInstruction> {
        let value = self.parsed.clone()?;
        serde_json::from_value(value).ok()
    }

    pub fn memo_text(&self) -> Option<String> {
        match &self.parsed {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            _ => None,
        }
    }
}

// ---- accounts --------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AccountInfoResult {
    pub value: Option<AccountValue>,
}

#[derive(Debug, Deserialize)]
pub struct AccountValue {
    pub data: AccountData,
    pub owner: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountData {
    pub parsed: ParsedAccount,
}

#[derive(Debug, Deserialize)]
pub struct ParsedAccount {
    #[serde(rename = "type")]
    pub kind: String,
    pub info: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenAccountInfo {
    pub mint: String,
    pub owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintInfo {
    pub decimals: u8,
    pub mint_authority: Option<String>,
}
