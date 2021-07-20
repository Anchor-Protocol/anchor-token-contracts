use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Already claimed")]
    AlreadyClaimed {},

    #[error("Invalid hex encoded proof")]
    InvalidHexProof {},

    #[error("Invalid hex encoded merkle root")]
    InvalidHexMerkle {},

    #[error("Merkle verification failed")]
    MerkleVerification {},

    #[error("Unauthorized")]
    Unauthorized {},
}
