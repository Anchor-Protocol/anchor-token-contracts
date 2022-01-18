use cosmwasm_std::StdError;
use thiserror::Error;

pub type ContractResult<T> = core::result::Result<T, ContractError>;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Cannot withdraw unclaimed refunds - last claim deadline not exceeded")]
    LastClaimDeadlineNotExceeded,

    #[error("Cannot withdraw refunds - last claim deadline exceeded")]
    LastClaimDeadlineExceeded,

    #[error("Invalid vesting schedule: {0}")]
    InvalidVestingSchedule(String),
}
