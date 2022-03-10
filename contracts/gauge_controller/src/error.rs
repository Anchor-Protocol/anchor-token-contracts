use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Gauge Not Found")]
    GaugeNotFound {},

    #[error("Gauge Already Exists")]
    GaugeAlreadyExists {},

    #[error("Deserialization Error")]
    DeserializationError {},

    #[error("Timestamp Error")]
    TimestampError {},

    #[error("Invalid Voting Ratio")]
    InvalidVotingRatio {},

    #[error("Lock Expires Too Soon")]
    LockExpiresTooSoon {},

    #[error("Vote Too Often")]
    VoteTooOften {},
}
